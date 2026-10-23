use std::time::Duration;
use tst_xiangqi::{
    engine::{MoveAssessment, Score, assess_move},
    game::{Game, MAX_REVISION},
    persistence::{Command, Response, Worker},
    rules::Side,
    session::{SaveState, SaveStatus, recovery_records},
    storage::{Data, SCHEMA_VERSION, Settings, Storage},
};

#[test]
fn failed_save_reload_resume_fork_and_restart_preserve_every_move() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let old = Game::default();
    store.save_game(old.snapshot(true, 0, false), None).unwrap();
    let mut latest = old.clone();
    latest.play("a3a4".parse().unwrap()).unwrap();
    latest.play("a6a5".parse().unwrap()).unwrap();
    let failed = latest.snapshot(true, 0, false);
    let mut saves = SaveState::default();
    let worker = Worker::new(store.clone());
    let previous = dir.path().join("previous.json");
    std::fs::rename(&store.path, &previous).unwrap();
    std::fs::create_dir(&store.path).unwrap();
    let command = Command::save(failed.clone(), None);
    let seq = worker.send(command.clone()).unwrap();
    saves.queued(seq, command);
    let event = worker.rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(event.result.is_err());
    saves.completed(event.seq);
    saves.failed(event.retry.unwrap());
    std::fs::remove_dir(&store.path).unwrap();
    std::fs::rename(previous, &store.path).unwrap();
    assert_eq!(saves.status(&old.id), SaveStatus::Failed);
    let seq = worker.send(Command::Load).unwrap();
    saves.queued(seq, Command::Load);
    let event = worker.rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let Response::Data(data) = event.result.unwrap() else {
        panic!("load response")
    };
    saves.succeeded(seq, &data);
    let loaded = saves.project(*data);
    let resumed = loaded.active.unwrap();
    assert_eq!(resumed.history.len(), 2);
    // Even an old page retained by navigation must not replace the failed snapshot.
    let candidates = recovery_records(&failed, Some(&old.snapshot(true, 0, false)));
    assert_eq!(candidates.len(), 1);
    let mut branch = candidates[0].replay().unwrap();
    branch.fork_identity();
    let branch_id = branch.id.clone();
    store
        .save_game(branch.snapshot(true, 0, false), None)
        .unwrap();
    let restarted = Storage::new(store.path).load().unwrap();
    assert_eq!(restarted.active.unwrap().id, branch_id);
    assert_eq!(restarted.drafts[0].history.len(), 0);
    assert_eq!(resumed.replay().unwrap().board, latest.board);
}

#[test]
fn divergent_recovery_keeps_both_histories_and_descendants_are_deduplicated() {
    let old = Game::default();
    let mut a = old.clone();
    let mut b = old.clone();
    a.play("a3a4".parse().unwrap()).unwrap();
    b.play("c3c4".parse().unwrap()).unwrap();
    let ra = a.snapshot(true, 0, false);
    let rb = b.snapshot(true, 0, false);
    assert_eq!(recovery_records(&ra, Some(&rb)).len(), 2);
    a.play("a6a5".parse().unwrap()).unwrap();
    let descendant = a.snapshot(true, 0, false);
    let recovered = recovery_records(&ra, Some(&descendant));
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].history.len(), 2);
}

#[test]
fn successful_metadata_remains_visible_behind_pending_and_failed_writes() {
    let mut game = Game::default();
    game.resign(Side::Red);
    let mut confirmed = Data::default();
    let mut record = game.snapshot(true, 0, false);
    record.favorite = true;
    record.title = "确认名称".into();
    confirmed.archives.push(record);
    let mut saves = SaveState::default();
    saves.queued(1, Command::Favorite(game.id.clone(), "op".into()));
    saves.queued(2, Command::Rename("missing".into(), "失败名称".into()));
    saves.succeeded(1, &confirmed);
    assert!(saves.project(confirmed.clone()).archives[0].favorite);
    let failed = saves.completed(2).unwrap();
    saves.failed(failed);
    let visible = saves.project(confirmed);
    assert!(visible.archives[0].favorite);
    assert_eq!(visible.archives[0].title, "确认名称");
}

#[test]
fn older_success_never_overwrites_a_pending_board() {
    let mut game = Game::default();
    let old = game.snapshot(true, 0, false);
    game.play("a3a4".parse().unwrap()).unwrap();
    let new = game.snapshot(true, 0, false);
    let mut saves = SaveState::default();
    saves.queued(1, Command::save(old.clone(), None));
    saves.queued(2, Command::save(new.clone(), None));
    let data = Data {
        active: Some(old),
        ..Default::default()
    };
    saves.succeeded(1, &data);
    assert_eq!(saves.project(data).active.unwrap().history.len(), 1);
    assert_eq!(saves.status(&game.id), SaveStatus::Pending);
    let data = Data {
        active: Some(new),
        ..Default::default()
    };
    saves.succeeded(2, &data);
    assert_eq!(saves.status(&game.id), SaveStatus::Saved);
}

#[test]
fn deletion_and_bulk_clear_reject_old_windows_but_allow_a_new_branch() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let mut old = Game::default();
    store.save_game(old.snapshot(true, 0, false), None).unwrap();
    store
        .save_game(Game::default().snapshot(true, 0, false), None)
        .unwrap();
    store.remove_by_id(&old.id).unwrap();
    assert!(store.save_game(old.snapshot(true, 0, false), None).is_err());
    old.fork_identity();
    store.save_game(old.snapshot(true, 0, false), None).unwrap();
    old.resign(Side::Red);
    store.save_game(old.snapshot(true, 0, false), None).unwrap();
    store
        .update(|d| {
            d.clear_archives();
            d.clear_drafts();
            Ok(())
        })
        .unwrap();
    assert!(store.save_game(old.snapshot(true, 0, false), None).is_err());
    assert!(store.load().unwrap().deleted_ids.contains(&old.id));
}

#[test]
fn settings_save_does_not_rewrite_or_parse_the_game_library() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    store
        .save_game(Game::default().snapshot(true, 0, false), None)
        .unwrap();
    let before = std::fs::read(&store.path).unwrap();
    let settings = Settings {
        sound: false,
        music_volume: 0.1,
        ..Default::default()
    };
    store.save_settings(settings.clone()).unwrap();
    assert_eq!(std::fs::read(&store.path).unwrap(), before);
    assert_eq!(store.load().unwrap().settings, settings);
    std::fs::write(&store.path, b"damaged game data").unwrap();
    store.save_settings(Settings::default()).unwrap();
    assert_eq!(std::fs::read(&store.path).unwrap(), b"damaged game data");
}

#[test]
fn settings_backup_and_future_guards_are_independent_of_game_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let first = Settings {
        sound: false,
        ..Default::default()
    };
    store.save_settings(first.clone()).unwrap();
    store.save_settings(Settings::default()).unwrap();
    std::fs::write(store.settings_path(), b"broken").unwrap();
    assert_eq!(store.load().unwrap().settings, first);
    store.save_settings(Settings::default()).unwrap();
    assert!(
        std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .any(|p| p.file_name().to_string_lossy().contains("damaged-"))
    );
    let future = br#"{"version":999,"settings":{}}"#;
    std::fs::write(store.settings_path(), future).unwrap();
    assert!(store.save_settings(Settings::default()).is_err());
    assert_eq!(std::fs::read(store.settings_path()).unwrap(), future);
}

#[test]
fn settings_worker_acknowledges_durability_and_returns_retryable_failures() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let worker = Worker::new(store.clone());
    worker.send(Command::Settings(Settings::default())).unwrap();
    let event = worker.rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(event.result, Ok(Response::Settings(_))));
    std::fs::remove_file(store.settings_path()).unwrap();
    std::fs::create_dir(store.settings_path()).unwrap();
    worker.send(Command::Settings(Settings::default())).unwrap();
    let event = worker.rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(event.result.is_err());
    assert!(matches!(event.retry, Some(Command::Settings(_))));
}

#[test]
fn mate_transitions_are_typed_for_both_players() {
    for side in [Side::Red, Side::Black] {
        let relative = |s: Score| s.red_relative(side);
        for (before, after, expected) in [
            (Score::Mate(3), Score::Cp(100), MoveAssessment::MissedMate),
            (Score::Cp(-100), Score::Mate(-3), MoveAssessment::AllowsMate),
            (
                Score::Mate(-3),
                Score::Cp(-100),
                MoveAssessment::EscapedMate,
            ),
            (Score::Cp(100), Score::Mate(3), MoveAssessment::FoundMate),
            (
                Score::Mate(3),
                Score::Mate(7),
                MoveAssessment::MateContinuation,
            ),
            (
                Score::Cp(150),
                Score::Cp(20),
                MoveAssessment::Centipawns(130),
            ),
        ] {
            assert_eq!(
                assess_move(relative(before), relative(after), side),
                expected
            );
        }
    }
}

#[test]
fn bad_revision_is_rejected_and_old_schema_still_migrates() {
    let mut record = Game::default().snapshot(true, 0, false);
    record.revision = u64::MAX;
    assert!(record.replay().is_err());
    record.revision = MAX_REVISION;
    let mut bounded = record.replay().unwrap();
    bounded.play("a3a4".parse().unwrap()).unwrap();
    assert!(bounded.snapshot(true, 0, false).replay().is_ok());
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let mut value = serde_json::to_value(Data::default()).unwrap();
    value["version"] = 4.into();
    value.as_object_mut().unwrap().remove("deleted_ids");
    std::fs::write(&store.path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(store.load().unwrap().version, SCHEMA_VERSION);
}

#[test]
fn render_pressure_has_hysteresis_and_mobile_supersampling_is_bounded() {
    let mut budget = tst_xiangqi::rendering::RenderBudget::default();
    let size = budget.size(390., 844., 1., 8192, true);
    assert!(u64::from(size.0) * u64::from(size.1) <= 390 * 844 * 23 / 10);
    let full = budget.size(1920., 1080., 1., 8192, false);
    budget.observe(80., 0.016);
    assert_eq!(budget.size(1920., 1080., 1., 8192, false), full);
    for _ in 0..150 {
        budget.observe(32., 0.016);
    }
    let reduced = budget.size(1920., 1080., 1., 8192, false);
    assert!(reduced.0 < full.0);
    for _ in 0..100 {
        budget.observe(5., 0.016);
    }
    assert_eq!(budget.size(1920., 1080., 1., 8192, false), reduced);
    for _ in 0..650 {
        budget.observe(5., 0.016);
    }
    assert_eq!(budget.size(1920., 1080., 1., 8192, false), full);
}

#[test]
fn chart_downsampling_keeps_tactical_spikes_and_unknown_gaps() {
    let mut scores = vec![Some(Score::Cp(0)); 4097];
    scores[123] = Some(Score::Mate(3));
    scores[124] = Some(Score::Mate(-3));
    scores[600] = None;
    let samples = tst_xiangqi::review::chart_samples(&scores, 300);
    assert!(samples.len() < 1500);
    for index in [123, 124, 600] {
        assert!(samples.contains(&(index, scores[index])));
    }
}

#[test]
fn unicode_native_replacement_and_missing_glyph_fallback_preserve_names() {
    let mut name = tst_xiangqi::text_input::NameEdit::from("旧名称");
    name.replace_native("中文😀𠮷".into(), 2, 4);
    assert_eq!(name.selected(), "😀𠮷");
    name.insert("残局");
    assert_eq!(name.text, "中文残局");
    name.replace_native("棋".repeat(60), 60, 60);
    assert_eq!(name.cursor, 40);
    let text = tst_xiangqi::typography::fallback_text("中文😀", |c| c != '😀');
    assert_eq!(text, "中文[U+1F600]");
}

#[test]
fn streaming_asset_install_validates_before_replacing_existing_bytes() {
    use sha2::{Digest, Sha256};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("engine.nnue");
    std::fs::write(&path, b"previous").unwrap();
    let bytes = vec![42u8; 200_000];
    let hash = format!("{:x}", Sha256::digest(&bytes));
    tst_xiangqi::platform::install_nnue_reader(&path, &mut std::io::Cursor::new(&bytes), &hash)
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(
        tst_xiangqi::platform::install_nnue_reader(
            &path,
            &mut std::io::Cursor::new(b"broken"),
            &hash
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn calibration_fixtures_cover_both_colors_and_reflected_move_counts() {
    let cases = tst_xiangqi::calibration::positions();
    assert_eq!(cases.len(), 10);
    for pair in cases.chunks(2) {
        assert_eq!(pair[0].1.board.turn, Side::Red);
        assert_eq!(pair[1].1.board.turn, Side::Black);
        assert!(!pair[0].1.board.legal_moves().is_empty());
        assert_eq!(
            pair[0].1.board.legal_moves().len(),
            pair[1].1.board.legal_moves().len()
        );
    }
}

#[test]
fn simplified_long_chase_and_mixed_checks_are_explicit_draws() {
    let mut chase = Game::new("5k3/9/9/9/9/9/9/R1n6/9/4K4 b - - 0 1".parse().unwrap());
    for mv in ["c2d4", "a2a4", "d4c2", "a4a2"].into_iter().cycle().take(8) {
        chase.play(mv.parse().unwrap()).unwrap();
    }
    assert_eq!(chase.outcome, tst_xiangqi::game::Outcome::Draw);
    assert!(chase.history.iter().all(|e| !e.check));
    let mut mixed = Game::new("5k3/9/9/9/5P3/9/9/9/3r5/4K4 b - - 0 1".parse().unwrap());
    for mv in [
        "d1e1", "e0d0", "e1f1", "d0e0", "f1c1", "e0f0", "c1d1", "f0e0",
    ]
    .into_iter()
    .cycle()
    .take(16)
    {
        mixed.play(mv.parse().unwrap()).unwrap();
    }
    assert_eq!(mixed.outcome, tst_xiangqi::game::Outcome::Draw);
    assert!(mixed.history.iter().any(|e| e.check));
}

#[test]
fn bare_kings_perft_and_arbitrary_fen_inputs_do_not_panic() {
    let board: tst_xiangqi::rules::Board = "3k5/9/9/9/9/9/9/9/9/5K3 w - - 0 1".parse().unwrap();
    assert_eq!(board.legal_moves().len(), 2);
    let next: usize = board
        .legal_moves()
        .into_iter()
        .map(|mv| {
            let mut b = board.clone();
            b.play(mv).unwrap();
            b.legal_moves().len()
        })
        .sum();
    assert_eq!(next, 3);
    let mut seed = 93u64;
    for length in 0..300 {
        let text: String = (0..length)
            .map(|_| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                char::from_u32(((seed >> 32) % 0x110000) as u32).unwrap_or('?')
            })
            .collect();
        let _ = text.parse::<tst_xiangqi::rules::Board>();
    }
}

#[test]
fn review_cache_and_async_analysis_do_not_move_the_viewer() {
    let game = Game::default();
    let mut review = tst_xiangqi::review::ReviewSession::new(game, true);
    let best = review.game.board.legal_moves()[0];
    let variation = tst_xiangqi::engine::Variation {
        mv: best,
        score: 10,
        depth: 3,
        index: 1,
        evaluation: Score::Cp(10),
    };
    review.analyzed(0, &variation);
    assert_eq!(review.ply, 0);
    let version = review.score_version;
    review.cache_loaded(Some(tst_xiangqi::review::CachedReview {
        entries: vec![Some(tst_xiangqi::review::CachedPosition {
            score: Score::Cp(20),
            best: Some(best.to_string()),
            depth: 5,
        })],
        updated_at: 0,
    }));
    assert!(!review.cache_loading);
    assert_eq!(review.depths[0], 5);
    assert!(review.score_version > version);
    assert_eq!(review.ply, 0);
    review.analyzed(999, &variation);
    assert_eq!(review.scores.len(), 1);
}

#[test]
fn semantic_control_identity_is_stable_and_bounds_are_preserved() {
    use tst_xiangqi::accessibility::Element;
    let first = Element::new("开始新对局".into(), [1., 2., 100., 44.], true);
    assert_eq!(
        first,
        Element::new("开始新对局".into(), [1., 2., 100., 44.], true)
    );
    assert_ne!(
        first.id,
        Element::new("确认删除".into(), first.rect, true).id
    );
    assert!(first.actionable);
    assert_eq!(first.rect[3], 44.);
    assert_eq!(
        Element::new("声音：开启".into(), first.rect, true).id,
        Element::new("声音：静音".into(), first.rect, true).id
    );
    assert_eq!(
        Element::new("第1列第1行，红方车".into(), first.rect, true).id,
        Element::new("第1列第1行，空位".into(), first.rect, true).id
    );
}
