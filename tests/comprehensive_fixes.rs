use tst_xiangqi::{
    engine::{Score, Variation},
    game::{Game, LEVELS},
    review::{ANALYSIS_LIMITS, AnalysisDetails, CachedPosition, CachedReview, ReviewSession},
    rules::Side,
    storage::Storage,
};

#[test]
fn analysis_uses_master_limits_and_never_downgrades_a_complete_result() {
    assert_eq!(ANALYSIS_LIMITS, (LEVELS[4].depth, LEVELS[4].ms));
    assert_eq!(
        LEVELS.iter().map(|l| (l.depth, l.ms)).collect::<Vec<_>>(),
        [(3, 1000), (5, 1500), (7, 2000), (9, 2500), (12, 3000)]
    );
    let entry = CachedPosition {
        score: Score::Cp(80),
        best: Some("h2e2".into()),
        depth: 12,
        details: None,
    };
    let mut review = ReviewSession::new(Game::default(), true);
    review.cache_loaded(Some(CachedReview {
        entries: vec![Some(entry.clone())],
        updated_at: 0,
    }));
    assert!(!review.complete(0));
    review.analyzed(
        0,
        &Variation {
            mv: "h2e2".parse().unwrap(),
            score: 10,
            depth: 3,
            index: 1,
            evaluation: Score::Cp(10),
            line: vec![],
        },
    );
    assert_eq!(review.scores[0], Some(Score::Cp(80)));
    assert_eq!(review.depths[0], 12);
    assert!(
        review.complete(0),
        "a budget-limited attempt must not loop forever"
    );
    let detailed = CachedPosition {
        details: Some(AnalysisDetails {
            line: vec!["h2e2".into()],
            elapsed_ms: 100,
            budget_ms: ANALYSIS_LIMITS.1,
            requested_depth: ANALYSIS_LIMITS.0,
        }),
        ..entry.clone()
    };
    assert!(detailed.supersedes(&entry));
    assert!(!entry.supersedes(&detailed));
    assert!(
        !CachedPosition {
            depth: 3,
            ..detailed.clone()
        }
        .supersedes(&detailed)
    );
}

#[test]
fn deletion_is_atomic_against_stale_versions_and_excludes_unseen_games() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let mut game = Game::default();
    game.resign(Side::Red);
    let old = game.snapshot(true, 0, false);
    store.save_game(old.clone(), None).unwrap();
    let mut other = Game::default();
    other.resign(Side::Black);
    store
        .save_game(other.snapshot(true, 0, false), None)
        .unwrap();
    assert!(
        store
            .remove_checked(&[(old.id.clone(), "stale".into())])
            .is_err()
    );
    assert_eq!(store.load().unwrap().archives.len(), 2);
    store
        .remove_checked(&[(old.id.clone(), old.version_token.clone())])
        .unwrap();
    let data = store.load().unwrap();
    assert_eq!(data.archives.len(), 1);
    assert_eq!(data.archives[0].id, other.id);
    store
        .remove_checked(&[(old.id, old.version_token)])
        .unwrap();
}

#[test]
fn repair_quarantines_invalid_games_and_never_downgrades_future_data() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let mut game = Game::default();
    game.resign(Side::Red);
    store
        .save_game(game.snapshot(true, 0, false), None)
        .unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&store.path).unwrap()).unwrap();
    value["archives"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"broken": true}));
    let broken = serde_json::to_vec(&value).unwrap();
    std::fs::write(&store.path, &broken).unwrap();
    assert!(store.load().is_err());
    assert_eq!(store.repair().unwrap().archives.len(), 1);
    assert!(
        std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| e.path().extension().is_some_and(|x| x == "json")
                && std::fs::read(e.path()).unwrap() == broken)
    );
    let future = br#"{"version":999}"#;
    std::fs::write(&store.path, future).unwrap();
    assert!(store.repair().is_err());
    assert_eq!(std::fs::read(&store.path).unwrap(), future);
}

#[test]
fn legacy_backups_remain_importable_without_creating_new_backups() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let mut game = Game::default();
    game.play("h2e2".parse().unwrap()).unwrap();
    store
        .save_game(game.snapshot(true, 0, false), None)
        .unwrap();
    let export = dir.path().join("legacy-export.json");
    std::fs::copy(&store.path, &export).unwrap();
    game.play("h9g7".parse().unwrap()).unwrap();
    store
        .save_game(game.snapshot(true, 0, false), None)
        .unwrap();
    assert!(!dir.path().join("backups").exists());
    let target_dir = tempfile::tempdir().unwrap();
    let target = Storage::new(target_dir.path().join("state.json"));
    let restored = target.import_backup(&export).unwrap();
    assert_eq!(restored.drafts[0].history.len(), 1);
    assert!(
        store.import_backup(&export).is_err(),
        "divergent content must not replace the current game"
    );
}

#[test]
fn asynchronous_branches_preserve_history_and_have_a_new_identity() {
    let mut game = Game::default();
    for word in ["h2e2", "h9g7", "h0g2", "b9c7"] {
        game.play(word.parse().unwrap()).unwrap();
    }
    let mut loader = tst_xiangqi::replay::Loader::default();
    let id = loader.branch(game.snapshot(true, 0, false), 2).unwrap();
    let reply = loader
        .rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(id, reply.id);
    assert!(reply.branch);
    let branch = reply.result.unwrap();
    assert_ne!(branch.id, game.id);
    assert_eq!(branch.history.len(), 2);
    assert_eq!(&branch.board, game.at_ply(2).unwrap());
}

#[test]
fn explicit_restore_recovers_deleted_games_without_duplicate_retries() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let mut game = Game::default();
    game.resign(Side::Red);
    let record = game.snapshot(true, 0, false);
    store.save_game(record.clone(), None).unwrap();
    let export = dir.path().join("legacy-export.json");
    std::fs::copy(&store.path, &export).unwrap();
    store
        .remove_checked(&[(record.id.clone(), record.version_token)])
        .unwrap();
    let restored = store.import_backup(&export).unwrap();
    assert_eq!(restored.archives.len(), 1);
    assert_ne!(restored.archives[0].id, record.id);
    let again = store.import_backup(&export).unwrap();
    assert_eq!(again.archives.len(), 1);
    assert_eq!(again.archives[0].id, restored.archives[0].id);
    assert!(again.deleted_ids.contains(&record.id));
}

#[test]
fn wheel_units_preserve_small_steps_and_volume_bounds() {
    use tst_xiangqi::interaction::{wheel_delta, wheel_volume};
    assert_eq!(wheel_delta(120., true), 1.);
    assert_eq!(wheel_delta(15., true), 0.125);
    assert_eq!(wheel_delta(-240., true), -2.);
    assert_eq!(wheel_delta(0.5, false), 0.5);
    assert_eq!(wheel_delta(f32::NAN, true), 0.);
    assert!((wheel_volume(0.5, wheel_delta(120., true)) - 0.55).abs() < 0.0001);
    assert_eq!(wheel_volume(0.99, 3.), 1.);
    assert_eq!(wheel_volume(0.01, -3.), 0.);
}

#[test]
fn wrapped_labels_preserve_complete_unicode_text_without_ellipsis() {
    let text = "四十字以内的棋谱名称与完整日期都应保留显示内容";
    let lines = tst_xiangqi::typography::wrap_text(text, 8., |s| s.chars().count() as f32);
    assert_eq!(lines.concat(), text);
    assert!(lines.iter().all(|line| line.chars().count() <= 8));
    assert_eq!(
        tst_xiangqi::typography::wrap_text("前段\n后段", 20., |s| s.len() as f32),
        ["前段", "后段"]
    );
}
