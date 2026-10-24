use tst_xiangqi::{game::Game, storage::Storage};
#[test]
fn concurrent_updates_replace_safely_and_toggle_latest_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    let workers: Vec<_> = (0..12)
        .map(|i| {
            let path = path.clone();
            std::thread::spawn(move || {
                let store = Storage::new(path);
                let mut game = Game::default();
                game.resign(tst_xiangqi::rules::Side::Red);
                let mut r = game.snapshot(true, 0, false);
                r.time = i;
                store
                    .update(|d| {
                        d.archives.push(r);
                        Ok(())
                    })
                    .unwrap();
            })
        })
        .collect();
    for w in workers {
        w.join().unwrap();
    }
    let store = Storage::new(path);
    assert_eq!(store.load().unwrap().archives.len(), 12);
    store.toggle_favorite(0, true).unwrap();
    let d = store.toggle_favorite(0, true).unwrap();
    assert!(!d.archives.iter().find(|r| r.time == 0).unwrap().favorite);
}
#[test]
fn corrupt_data_is_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    std::fs::write(&path, "{broken").unwrap();
    let store = Storage::new(path.clone());
    assert!(
        store
            .update(|d| {
                d.active = None;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), "{broken");
}
#[test]
fn archive_and_active_change_in_one_transaction_and_import_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let mut g = Game::default();
    g.play("h2e2".parse().unwrap()).unwrap();
    let active = g.snapshot(true, 0, false);
    assert!(store.save_game(active, None).unwrap().active.is_some());
    g.resign(tst_xiangqi::rules::Side::Red);
    let record = g.snapshot(true, 0, false);
    let data = store.save_game(record.clone(), None).unwrap();
    assert!(data.active.is_none());
    assert_eq!(data.archives.len(), 1);
    let old = dir.path().join("legacy.json");
    std::fs::write(&old, serde_json::to_vec(&vec![record]).unwrap()).unwrap();
    assert_eq!(store.import_legacy(&old).unwrap().1, 0);
    assert!(old.is_file());
}
