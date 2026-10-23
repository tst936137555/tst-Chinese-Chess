use std::time::Duration;
use tst_xiangqi::{
    engine::Score,
    game::Game,
    persistence::{Command, Response, Worker},
    review::CachedPosition,
    rules::Side,
    session::SaveState,
    storage::{Data, Settings, Storage},
};

fn receive(worker: &Worker) -> Data {
    match worker
        .rx
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .result
        .unwrap()
    {
        Response::Data(data) => *data,
        _ => panic!("expected saved data"),
    }
}

#[test]
fn invalid_import_or_write_never_breaks_an_existing_readable_save() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let record = Game::default().snapshot(true, 0, false);
    store.save_game(record.clone(), None).unwrap();
    let before = std::fs::read(&store.path).unwrap();
    let input = dir.path().join("import.json");
    std::fs::write(
        &input,
        format!(r#"[{{"time":1,"history":[],"mode":"{}"}}]"#, "x".repeat(33)),
    )
    .unwrap();
    assert!(store.import_legacy(&input).is_err());
    let mut bad = record.clone();
    bad.mode = "x".repeat(33);
    assert!(store.save_game(bad.clone(), None).is_err());
    assert!(
        store
            .update(|d| {
                d.active = Some(bad);
                Ok(())
            })
            .is_err()
    );
    assert_eq!(std::fs::read(&store.path).unwrap(), before);
    assert_eq!(store.load().unwrap().active.unwrap().id, record.id);
    for mode in ["normal", "review", "older-custom-mode"] {
        let mut valid = record.clone();
        valid.mode = mode.into();
        store.save_game(valid, None).unwrap();
        assert_eq!(store.load().unwrap().active.unwrap().mode, mode);
    }
}

#[test]
fn archived_names_survive_reload_favorite_and_an_older_game_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let mut game = Game::default();
    game.play("h2e2".parse().unwrap()).unwrap();
    game.resign(Side::Black);
    let snapshot = game.snapshot(true, 0, false);
    store.save_game(snapshot.clone(), None).unwrap();
    let worker = Worker::new(store.clone());
    worker
        .send(Command::Rename(game.id.clone(), "  首局复盘  ".into()))
        .unwrap();
    assert_eq!(receive(&worker).archives[0].title, "首局复盘");
    store.favorite_by_id(&game.id).unwrap();
    store.save_game(snapshot, None).unwrap();
    let saved = store.load().unwrap().archives.remove(0);
    assert_eq!(saved.title, "首局复盘");
    assert!(saved.favorite);
    worker
        .send(Command::Rename(game.id, String::new()))
        .unwrap();
    assert!(receive(&worker).archives[0].title.is_empty());
}

#[test]
fn successful_new_settings_clear_stale_failures_without_dropping_moves() {
    let mut saves = SaveState::default();
    let record = Game::default().snapshot(true, 0, false);
    saves.failed(Command::Settings(Settings::default()));
    saves.failed(Command::save(record.clone(), Some(Settings::default())));
    let settings = Settings {
        sound: false,
        ..Default::default()
    };
    saves.queued(1, Command::Settings(settings.clone()));
    let mut data = Data {
        settings,
        ..Default::default()
    };
    saves.succeeded(1, &data);
    assert_eq!(saves.pending, 0);
    assert_eq!(saves.failed.len(), 1);
    assert!(matches!(&saves.failed[0], Command::Save(r, None) if r.id == record.id));
    data.active = Some(record);
    saves.acknowledge(&data);
    assert!(saves.failed.is_empty());
}

#[test]
fn cache_rejects_zero_mate_and_unproven_terminal_scores() {
    let game = Game::default();
    for score in [Score::Mate(0), Score::Terminal(1), Score::Terminal(-1)] {
        assert!(
            !CachedPosition {
                score,
                best: Some("h2e2".into()),
                depth: 12
            }
            .valid_for(&game, 0)
        );
    }
    assert!(
        CachedPosition {
            score: Score::Cp(42),
            best: Some("h2e2".into()),
            depth: 12
        }
        .valid_for(&game, 0)
    );
    let terminal = Game::new("3k5/2R1R4/9/9/9/9/9/9/9/4K4 b - - 0 1".parse().unwrap());
    assert!(
        CachedPosition {
            score: Score::Terminal(1),
            best: None,
            depth: 0
        }
        .valid_for(&terminal, 0)
    );
    assert!(
        !CachedPosition {
            score: Score::Terminal(-1),
            best: None,
            depth: 0
        }
        .valid_for(&terminal, 0)
    );
}

#[test]
fn backup_recovery_worker_restores_data_and_keeps_the_damaged_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let record = Game::default().snapshot(true, 0, false);
    store.save_game(record.clone(), None).unwrap();
    store.update(|_| Ok(())).unwrap();
    std::fs::write(&store.path, "broken").unwrap();
    let worker = Worker::new(store.clone());
    worker.send(Command::RestoreBackup).unwrap();
    assert_eq!(receive(&worker).active.unwrap().id, record.id);
    assert_eq!(
        std::fs::read_to_string(store.path.with_extension("damaged.json")).unwrap(),
        "broken"
    );
    assert!(store.load().is_ok());
}
