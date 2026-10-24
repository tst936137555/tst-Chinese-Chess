use tst_xiangqi::{
    accessibility::Element,
    engine::{Score, Variation},
    game::{Game, RULESET, Record},
    persistence::{Command, Response, Worker},
    review::{CachedPosition, CachedReview, ReviewSession},
    rules::Side,
    storage::{Settings, SettingsChange, Storage},
};

fn finished() -> Record {
    let mut game = Game::default();
    game.resign(Side::Red);
    game.snapshot(true, 0, false)
}

#[test]
fn semantic_corruption_never_rotates_a_good_backup_or_accepts_unrelated_writes() {
    for variant in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let store = Storage::new(dir.path().join("state.json"));
        store.save_game(finished(), None).unwrap();
        store.update(|_| Ok(())).unwrap();
        let backup = std::fs::read(store.path.with_extension("bak")).unwrap();
        let mut data = store.load().unwrap();
        match variant {
            0 => data.archives[0].start_fen = Some("broken".into()),
            1 => data.archives[0]
                .history
                .push(tst_xiangqi::game::SavedMove { uci: "a0a9".into() }),
            _ => data.archives[0].end_reason = None,
        }
        let bytes = serde_json::to_vec(&data).unwrap();
        std::fs::write(&store.path, &bytes).unwrap();
        assert!(store.load().is_err());
        assert!(
            store
                .update(|d| {
                    d.archives[0].title = "rename".into();
                    Ok(())
                })
                .is_err()
        );
        assert!(store.save_game(finished(), None).is_err());
        assert_eq!(std::fs::read(&store.path).unwrap(), bytes);
        assert_eq!(
            std::fs::read(store.path.with_extension("bak")).unwrap(),
            backup
        );
        assert!(store.restore_backup().is_ok());
    }
}

#[test]
fn import_capacity_is_transactional_and_does_not_retire_rejected_records() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let original = finished();
    store.save_game(original.clone(), None).unwrap();
    let before = std::fs::read(&store.path).unwrap();
    let mut records: Vec<_> = (0..100).map(|_| finished()).collect();
    let source = dir.path().join("import.json");
    std::fs::write(&source, serde_json::to_vec(&records).unwrap()).unwrap();
    assert!(store.import_legacy(&source).err().unwrap().contains("上限"));
    assert_eq!(std::fs::read(&store.path).unwrap(), before);
    assert!(store.load().unwrap().deleted_ids.is_empty());
    records.pop();
    std::fs::write(&source, serde_json::to_vec(&records).unwrap()).unwrap();
    let (data, count) = store.import_legacy(&source).unwrap();
    assert_eq!(count, 99);
    assert_eq!(data.archives.len(), 100);
    assert!(data.archives.iter().any(|r| r.id == original.id));
    assert_eq!(store.import_legacy(&source).unwrap().1, 0);
}

#[test]
fn old_cache_and_equal_depth_cache_cannot_replace_live_analysis() {
    let mut review = ReviewSession::new(Game::default(), true);
    review.analyzed(
        0,
        &Variation {
            mv: "h2e2".parse().unwrap(),
            score: 120,
            depth: 12,
            index: 1,
            evaluation: Score::Cp(120),
        },
    );
    for depth in [3, 12] {
        review.cache_loaded(Some(CachedReview {
            entries: vec![Some(CachedPosition {
                score: Score::Cp(10),
                best: Some("h2e2".into()),
                depth,
            })],
            updated_at: 0,
        }));
        assert_eq!(review.depths[0], 12);
        assert_eq!(review.scores[0], Some(Score::Cp(120)));
    }
    assert!(review.dirty.contains(&0));
    assert_eq!(review.ply, 0);
}

#[test]
fn favorite_retries_keep_intent_and_reject_stale_intervening_edits() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let record = finished();
    let id = record.id.clone();
    store.save_game(record, None).unwrap();
    store.set_favorite(&id, true, 0).unwrap();
    store.set_favorite(&id, true, 0).unwrap();
    store.set_favorite(&id, false, 1).unwrap();
    assert!(store.set_favorite(&id, true, 0).is_err());
    let data = store.load().unwrap();
    assert!(!data.archives[0].favorite);
    assert_eq!(data.archives[0].favorite_revision, 2);
    assert!(data.applied_operations.is_empty());
}

#[test]
fn legacy_favorite_retry_survives_more_than_128_other_operations() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let record = finished();
    let id = record.id.clone();
    store.save_game(record, None).unwrap();
    store.favorite_once(&id, "original").unwrap();
    for i in 0..129 {
        store.favorite_once(&id, &format!("op-{i}")).unwrap();
    }
    let before = store.load().unwrap().archives[0].favorite;
    assert_eq!(
        store.favorite_once(&id, "original").unwrap().archives[0].favorite,
        before
    );
}

#[test]
fn preferences_can_load_when_game_library_cannot() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let settings = Settings {
        sound: false,
        music_volume: 0.11,
        ..Settings::default()
    };
    store.save_settings(settings.clone()).unwrap();
    std::fs::write(&store.path, b"broken").unwrap();
    assert!(store.load().is_err());
    assert_eq!(store.load_preferences(Settings::default()), settings);
}

#[test]
fn merged_preferences_preserve_other_fields_and_an_explicit_round_trip() {
    let base = Settings::default();
    let mut first = base.clone();
    first.sound = false;
    let mut last = first.clone();
    last.sound = true;
    last.effects_volume = 0.4;
    let mut change = SettingsChange {
        before: base.clone(),
        after: first.clone(),
    };
    change.merge(&SettingsChange {
        before: first,
        after: last,
    });
    let mut remote = Settings {
        music_volume: 0.9,
        sound: false,
        ..base
    };
    change.apply(&mut remote);
    assert!(remote.sound);
    assert_eq!(remote.music_volume, 0.9);
    assert_eq!(remote.effects_volume, 0.4);
}

#[test]
fn settings_queue_acknowledges_every_coalesced_patch_and_flushes_final_intent() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let worker = Worker::new(store.clone());
    let mut before = Settings::default();
    let mut sequences = std::collections::BTreeSet::new();
    for i in 0..50 {
        let mut after = before.clone();
        after.music_volume = i as f32 / 50.;
        sequences.insert(
            worker
                .send(Command::SettingsPatch(SettingsChange {
                    before: before.clone(),
                    after: after.clone(),
                }))
                .unwrap(),
        );
        before = after;
    }
    let flush = worker.send(Command::Flush).unwrap();
    loop {
        let event = worker
            .rx
            .recv_timeout(std::time::Duration::from_secs(15))
            .unwrap();
        match event.result.unwrap() {
            Response::Settings(_) | Response::Superseded => {
                assert!(sequences.remove(&event.seq));
            }
            Response::Flushed if event.seq == flush => break,
            _ => {}
        }
    }
    assert!(sequences.is_empty());
    assert_eq!(store.load().unwrap().settings.music_volume, 0.98);
}

#[test]
fn ruleset_migration_preserves_old_games_and_guards_future_rules() {
    let mut value = serde_json::to_value(finished()).unwrap();
    value.as_object_mut().unwrap().remove("ruleset");
    let record: Record = serde_json::from_value(value).unwrap();
    assert_eq!(record.ruleset, RULESET);
    assert!(record.replay().is_ok());
    let mut future = record;
    future.ruleset = "future-v2".into();
    assert!(future.replay().is_err());
}

#[test]
fn semantic_identity_survives_layout_and_label_changes_and_separates_records() {
    let first = Element::with_identity(
        "archive:a:favorite",
        "收藏".into(),
        [1., 2., 40., 40.],
        true,
    );
    let moved = Element::with_identity(
        "archive:a:favorite",
        "取消收藏".into(),
        [100., 200., 60., 60.],
        true,
    );
    let other = Element::with_identity("archive:b:favorite", first.label.clone(), first.rect, true);
    assert_eq!(first.id, moved.id);
    assert_ne!(first.id, other.id);
    assert_eq!(
        Element::new("声音：开启".into(), first.rect, true).id,
        Element::new("声音：静音".into(), moved.rect, true).id
    );
}

#[test]
fn extended_offline_history_still_proves_its_saved_ancestor() {
    let mut game = Game::default();
    let saved = game.snapshot(true, 0, false);
    for _ in 0..150 {
        game.play("h2e2".parse().unwrap()).unwrap();
        game.undo_to_player(Side::Red);
    }
    assert!(game.snapshot(true, 0, false).acknowledges(&saved));
    assert_eq!(game.history.len(), 0);
}

#[test]
fn compacted_deletions_still_bar_stale_windows_and_allow_new_branches() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let old = Game::default();
    let stale = old.snapshot(true, 0, false);
    let data = store
        .update(|d| {
            for _ in 0..4100 {
                d.deleted_ids.insert(tst_xiangqi::game::new_id());
            }
            d.deleted_ids.insert(stale.id.clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(data.deleted_ids.len(), 4096);
    assert!(data.retired_before_nanos > 0);
    assert!(!data.deleted_ids.contains(&stale.id));
    assert!(store.save_game(stale, None).is_err());
    assert!(
        store
            .save_game(old.branch_at(0).unwrap().snapshot(true, 0, false), None)
            .is_ok()
    );
}

#[test]
fn close_only_accepts_its_own_flush_and_waits_for_durable_game_saves() {
    use tst_xiangqi::session::CloseState;
    let mut close = CloseState::default();
    close.flushed(1);
    assert!(!close.can_exit(0, false));
    close.request();
    close.queued_flush(7);
    close.flushed(1);
    assert!(!close.can_exit(0, false));
    close.flushed(7);
    assert!(!close.can_exit(1, false));
    assert!(!close.can_exit(0, true));
    assert!(close.can_exit(0, false));
    close.cancel();
    assert!(!close.waiting());
}

#[test]
fn a_new_favorite_intent_supersedes_failed_stale_retries() {
    use tst_xiangqi::session::SaveState;
    let mut state = SaveState::default();
    state.failed(Command::SetFavorite {
        id: "game".into(),
        favorite: true,
        revision: 0,
    });
    state.queued(
        1,
        Command::SetFavorite {
            id: "game".into(),
            favorite: false,
            revision: 2,
        },
    );
    assert!(state.failed.is_empty());
    state.failed(Command::SetFavorite {
        id: "game".into(),
        favorite: true,
        revision: 0,
    });
    assert!(state.failed.is_empty());
}
