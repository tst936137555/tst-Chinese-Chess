use tst_xiangqi::{
    game::{Game, Outcome},
    rules::{Board, Side},
    storage::{Data, Settings, SettingsChange, Storage},
};
fn finished() -> tst_xiangqi::game::Record {
    let mut game = Game::default();
    game.resign(Side::Red);
    game.snapshot(true, 0, false)
}
#[test]
fn ordinary_moves_and_field_edits_do_not_revert_another_window() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let old = Settings::default();
    let mut remote = old.clone();
    remote.music_volume = 0.1;
    store.save_settings(remote.clone()).unwrap();
    store
        .save_game(Game::default().snapshot(true, 0, false), None)
        .unwrap();
    let mut edited = old.clone();
    edited.level = 3;
    let merged = store
        .patch_settings(&SettingsChange {
            before: old,
            after: edited,
        })
        .unwrap();
    assert_eq!(merged.music_volume, 0.1);
    assert_eq!(merged.level, 3);
    assert_eq!(store.load().unwrap().settings, merged);
}
#[test]
fn older_preference_ack_keeps_inflight_edits_and_remote_untouched_fields() {
    use tst_xiangqi::{persistence::Command, session::SaveState};
    let base = Settings::default();
    let mut first = base.clone();
    first.music_volume = 0.4;
    let mut later = first.clone();
    later.music_volume = 0.7;
    let mut saves = SaveState::default();
    saves.queued(
        1,
        Command::SettingsPatch(SettingsChange {
            before: base,
            after: first.clone(),
        }),
    );
    saves.queued(
        2,
        Command::SettingsPatch(SettingsChange {
            before: first.clone(),
            after: later,
        }),
    );
    let mut data = Data {
        settings: first,
        ..Default::default()
    };
    data.settings.effects_enabled = false;
    saves.succeeded(1, &data);
    let visible = saves.project_preferences(data.settings);
    assert_eq!(visible.music_volume, 0.7);
    assert!(!visible.effects_enabled);
}
#[test]
fn eviction_retires_both_plain_and_favorite_identities() {
    for favorite in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let store = Storage::new(dir.path().join("state.json"));
        let mut original = Game::default();
        let stale = original.snapshot(true, 0, false);
        original.resign(Side::Red);
        let mut retired = original.snapshot(true, 0, false);
        retired.favorite = favorite;
        store
            .update(|data| {
                for _ in 0..if favorite { 49 } else { 99 } {
                    let mut r = finished();
                    r.favorite = favorite;
                    data.archives.push(r);
                }
                data.archives.push(retired);
                Ok(())
            })
            .unwrap();
        let mut next = finished();
        next.favorite = favorite;
        let saved = store.save_game(next, None).unwrap();
        assert!(saved.deleted_ids.contains(&stale.id));
        assert!(store.save_game(stale.clone(), None).is_err());
        let branch = stale.replay().unwrap().branch_at(0).unwrap();
        assert!(
            store
                .save_game(branch.snapshot(true, 0, false), None)
                .is_ok()
        );
    }
}
#[test]
fn malformed_library_relationships_never_overwrite_existing_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let record = Game::default().snapshot(true, 0, false);
    for variant in 0..4 {
        let mut data = Data {
            active: Some(record.clone()),
            ..Default::default()
        };
        match variant {
            0 => data.drafts.push(record.clone()),
            1 => data.archives.push(record.clone()),
            2 => {
                data.deleted_ids.insert(record.id.clone());
            }
            _ => data.active = Some(finished()),
        }
        let bytes = serde_json::to_vec(&data).unwrap();
        std::fs::write(&store.path, &bytes).unwrap();
        assert!(store.load().is_err());
        assert!(store.update(|_| Ok(())).is_err());
        assert_eq!(std::fs::read(&store.path).unwrap(), bytes);
    }
}
#[test]
fn preference_retries_retain_other_fields_and_clear_superseded_failures() {
    use tst_xiangqi::{persistence::Command, session::SaveState};
    let base = Settings::default();
    let mut volume = base.clone();
    volume.music_volume = 0.5;
    let mut state = SaveState::default();
    state.failed(Command::SettingsPatch(SettingsChange {
        before: base.clone(),
        after: volume.clone(),
    }));
    let mut muted = base.clone();
    muted.sound = false;
    state.queued(
        1,
        Command::SettingsPatch(SettingsChange {
            before: base.clone(),
            after: muted.clone(),
        }),
    );
    let mut data = Data {
        settings: muted,
        ..Default::default()
    };
    state.succeeded(1, &data);
    assert_eq!(state.failed.len(), 1);
    let mut final_volume = base.clone();
    final_volume.music_volume = 0.7;
    state.queued(
        2,
        Command::SettingsPatch(SettingsChange {
            before: volume,
            after: final_volume.clone(),
        }),
    );
    data.settings.music_volume = 0.7;
    state.succeeded(2, &data);
    assert!(state.failed.is_empty());
}
#[test]
fn failed_preferences_keep_future_schema_and_do_not_rewrite_the_library() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    std::fs::write(&store.path, b"broken library").unwrap();
    let before = Settings::default();
    let mut after = before.clone();
    after.sound = false;
    let change = SettingsChange { before, after };
    store.patch_settings(&change).unwrap();
    assert_eq!(std::fs::read(&store.path).unwrap(), b"broken library");
    let future = br#"{"version":99,"settings":{}}"#;
    std::fs::write(store.settings_path(), future).unwrap();
    assert!(store.patch_settings(&change).is_err());
    assert_eq!(std::fs::read(store.settings_path()).unwrap(), future);
}
#[test]
fn long_history_validates_only_new_moves_and_rejects_divergence() {
    let start = Board::default();
    let mut board = start.clone();
    let mut moves = Vec::new();
    let mut verifier = tst_xiangqi::history::HistoryVerifier::default();
    let mut checked = 0;
    for i in 0..4096 {
        let mv = ["b0c2", "b9c7", "c2b0", "c7b9"][i % 4].parse().unwrap();
        board.play(mv).unwrap();
        moves.push(mv);
        checked += verifier.verify("long", &start, &moves, &board).unwrap();
    }
    assert_eq!(checked, 4096);
    assert_eq!(verifier.verify("long", &start, &moves, &board).unwrap(), 0);
    assert!(verifier.verify("long", &start, &moves, &start).is_err());
    let mut fork = start.clone();
    let mv = "h2e2".parse().unwrap();
    fork.play(mv).unwrap();
    assert_eq!(verifier.verify("long", &start, &[mv], &fork).unwrap(), 1);
}
#[test]
fn failed_operation_copy_names_the_affected_resource() {
    use tst_xiangqi::{persistence::Command, session::SaveState};
    let mut state = SaveState::default();
    state.failed(Command::Settings(Settings::default()));
    assert!(state.failure_description().0.contains("设置"));
    state.failed.clear();
    state.failed(Command::Rename("test".into(), "名称".into()));
    assert!(state.failure_description().0.contains("名称"));
}
#[cfg(any(target_os = "windows", target_os = "macos"))]
#[test]
fn native_focus_is_distinct_from_invoking_the_control() {
    use tst_xiangqi::accessibility::{ControlAction, desktop_action, take_action};
    desktop_action(accesskit::Action::Focus, 2468);
    assert_eq!(take_action(), Some(ControlAction::Focus(2468)));
    assert_eq!(take_action(), None);
    desktop_action(accesskit::Action::Click, 2468);
    assert_eq!(take_action(), Some(ControlAction::Click(2468)));
}
#[test]
fn unicode_ellipsis_uses_few_measurements_and_pacing_stays_separate() {
    let calls = std::cell::Cell::new(0);
    let text = "棋😀局".repeat(1000);
    let clipped = tst_xiangqi::typography::ellipsize(&text, 40., |s| {
        calls.set(calls.get() + 1);
        s.chars().count() as f32
    });
    assert!(clipped.ends_with('…'));
    assert_eq!(clipped.chars().count(), 40);
    assert!(calls.get() < 20);
    assert_eq!(tst_xiangqi::session::minimum_think_seconds(4), 1.);
    assert_eq!(tst_xiangqi::game::LEVELS[4].depth, 12);
    assert_eq!(Outcome::Playing, Game::default().outcome);
}
#[test]
fn cache_deltas_survive_restart_without_dropping_deeper_results() {
    use tst_xiangqi::{
        engine::Score,
        persistence::{Command, Response, Worker},
        review::CachedPosition,
    };
    if tst_xiangqi::persistence::asset_fingerprint().is_err() {
        assert!(std::env::var_os("XIANGQI_REQUIRE_ENGINE").is_none());
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let worker = Worker::new(store.clone());
    for (ply, depth) in [(0, 12), (1, 12), (0, 3)] {
        worker
            .send(Command::CacheUpdate {
                key: "delta".into(),
                len: 4097,
                ply,
                entry: CachedPosition {
                    score: Score::Cp(depth as i32),
                    depth,
                    best: Some("h2e2".into()),
                },
            })
            .unwrap();
    }
    worker.send(Command::Flush).unwrap();
    loop {
        if matches!(
            worker
                .rx
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap()
                .result
                .unwrap(),
            Response::Flushed
        ) {
            break;
        }
    }
    drop(worker);
    let worker = Worker::new(store);
    worker
        .send(Command::CacheLoad {
            key: "delta".into(),
        })
        .unwrap();
    if let Response::Cache {
        review: Some(review),
        ..
    } = worker
        .rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap()
        .result
        .unwrap()
    {
        assert_eq!(review.entries[0].as_ref().unwrap().depth, 12);
        assert!(review.entries[1].is_some());
        assert!(review.entries[4096].is_none());
    } else {
        panic!("missing delta cache");
    }
}
