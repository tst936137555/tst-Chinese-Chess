use std::{fs, time::Duration};
use tst_xiangqi::{
    game::Game,
    persistence::{Command, Response, Worker},
    rules::{Board, Side},
    storage::{Settings, Storage},
};

#[test]
fn missing_primary_protects_every_backup_until_explicit_recovery() {
    for bytes in [b"not json".as_slice(), br#"{"version":999}"#.as_slice()] {
        let dir = tempfile::tempdir().unwrap();
        let store = Storage::new(dir.path().join("state.json"));
        fs::write(dir.path().join("state.bak"), bytes).unwrap();
        assert!(store.load().is_err());
        assert!(
            store
                .save_game(Game::default().snapshot(true, 0, false), None)
                .is_err()
        );
        assert_eq!(fs::read(dir.path().join("state.bak")).unwrap(), bytes);
        assert!(!store.path.exists());
        assert!(store.restore_backup().is_err());
    }
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let mut game = Game::default();
    game.resign(Side::Red);
    store
        .save_game(game.snapshot(true, 0, false), None)
        .unwrap();
    store.update(|_| Ok(())).unwrap();
    fs::remove_file(&store.path).unwrap();
    assert!(store.load().is_err());
    assert_eq!(store.restore_backup().unwrap().archives[0].id, game.id);
    assert_eq!(store.load().unwrap().archives.len(), 1);
}

#[test]
fn missing_settings_recovers_and_guards_a_future_backup() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let settings = Settings {
        sound: false,
        ..Default::default()
    };
    store.save_settings(settings.clone()).unwrap();
    store
        .save_settings(Settings {
            music_volume: 0.5,
            ..settings.clone()
        })
        .unwrap();
    fs::remove_file(store.settings_path()).unwrap();
    assert!(!store.load_preferences(Settings::default()).sound);
    let backup = store.settings_path().with_extension("bak");
    let future = br#"{"version":999,"settings":{}}"#;
    fs::write(&backup, future).unwrap();
    assert!(store.save_settings(Settings::default()).is_err());
    assert_eq!(fs::read(backup).unwrap(), future);
    assert!(!store.settings_path().exists());
}

fn response(worker: &Worker, seq: u64) -> Result<Response, String> {
    loop {
        let event = worker.rx.recv_timeout(Duration::from_secs(5)).unwrap();
        if event.seq == seq {
            return event.result;
        }
    }
}
#[test]
fn names_reject_stale_retries_and_survive_moves() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let mut game = Game::default();
    store
        .save_game(game.snapshot(true, 0, false), None)
        .unwrap();
    let worker = Worker::new(store.clone());
    for (title, revision) in [("first", 0), ("newer", 1)] {
        let seq = worker
            .send(Command::Rename(game.id.clone(), title.into(), revision))
            .unwrap();
        assert!(response(&worker, seq).is_ok());
    }
    let retry = worker
        .send(Command::Rename(game.id.clone(), "first".into(), 0))
        .unwrap();
    assert!(response(&worker, retry).err().unwrap().contains("conflict"));
    let same = worker
        .send(Command::Rename(game.id.clone(), "newer".into(), 1))
        .unwrap();
    assert!(response(&worker, same).is_ok());
    game.play("h2e2".parse().unwrap()).unwrap();
    let saved = store
        .save_game(game.snapshot(true, 0, false), None)
        .unwrap()
        .active
        .unwrap();
    assert_eq!((saved.title.as_str(), saved.title_revision), ("newer", 2));
}

#[test]
fn replay_returns_a_live_game_with_original_identity_and_lineage() {
    let mut game = Game::default();
    for word in ["h2e2", "h9g7", "h0g2", "b9c7"] {
        game.play(word.parse().unwrap()).unwrap();
    }
    let record = game.snapshot(true, 0, false);
    let mut restored = record.replay().unwrap();
    assert_eq!(restored.board, game.board);
    assert_eq!(restored.lineage, game.lineage);
    assert_eq!(restored.version_token, game.version_token);
    restored.play("i0h0".parse().unwrap()).unwrap();
    assert!(restored.snapshot(true, 0, false).acknowledges(&record));
    for (_, fixture) in tst_xiangqi::calibration::positions() {
        assert_eq!(
            fixture.board.has_legal_move(),
            !fixture.board.legal_moves().is_empty()
        );
    }
}

#[test]
fn principal_variations_are_bounded_and_legal() {
    let board = Board::default();
    let v =
        tst_xiangqi::engine::parse_info("info depth 12 score cp 20 pv h2e2 h9g7 h0g2 a0a9", &board)
            .unwrap();
    assert_eq!(
        v.line.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["h2e2", "h9g7", "h0g2"]
    );
    let mut cache = tst_xiangqi::review::CachedPosition {
        score: v.evaluation,
        best: Some(v.mv.to_string()),
        depth: v.depth,
        details: Some(tst_xiangqi::review::AnalysisDetails {
            line: vec!["h2e2".into(), "h9g7".into()],
            elapsed_ms: 80,
            budget_ms: 3000,
            requested_depth: 12,
        }),
    };
    assert!(cache.valid_for(&Game::default(), 0));
    cache.details.as_mut().unwrap().line.push("a0a9".into());
    assert!(!cache.valid_for(&Game::default(), 0));
}

#[test]
fn background_loader_preserves_request_identity() {
    let mut loader = tst_xiangqi::replay::Loader::default();
    let game = Game::default();
    let id = loader.request(game.snapshot(true, 0, false), true).unwrap();
    let reply = loader.rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(reply.id, id);
    assert!(reply.review);
    assert_eq!(reply.result.unwrap().id, game.id);
}

#[test]
fn legacy_operation_capacity_preserves_retries_without_growing() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let mut game = Game::default();
    game.resign(Side::Red);
    store
        .save_game(game.snapshot(true, 0, false), None)
        .unwrap();
    store
        .update(|d| {
            d.applied_operations = (0..4096).map(|i| format!("op-{i}")).collect();
            Ok(())
        })
        .unwrap();
    assert!(store.favorite_once(&game.id, "op-0").is_ok());
    assert!(store.favorite_once(&game.id, "new-operation").is_err());
    assert_eq!(store.load().unwrap().applied_operations.len(), 4096);
    assert!(store.set_favorite(&game.id, true, 0).is_ok());
}

#[test]
fn first_failure_is_saved_locally_with_paths_redacted() {
    let dir = tempfile::tempdir().unwrap();
    tst_xiangqi::diagnostics::enable_local(dir.path());
    let root = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let message = format!("failed at {root}/private/state.json");
    tst_xiangqi::diagnostics::failure(
        tst_xiangqi::diagnostics::FailureKind::Storage,
        "test",
        &message,
    );
    let path = dir.path().join("last-failure.txt");
    let until = std::time::Instant::now() + Duration::from_secs(5);
    while !path.exists() && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    let text = fs::read_to_string(path).unwrap();
    assert!(text.starts_with("version="));
    if !root.is_empty() {
        assert!(!text.contains(&root));
    }
    assert!(text.len() < 16_384);
}
