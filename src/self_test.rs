//! Headless installed-package smoke check; never opens or changes the user's save file.
pub fn run() -> Result<(), String> {
    use crate::{engine::Engine, game::Game, rules::Side, storage::Storage};
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let store = Storage::new(dir.path().join("state.json"));
    let mut game = Game::default();
    for mv in ["h2e2", "h9g7"] {
        game.play(mv.parse()?)?;
    }
    store.save_game(game.snapshot(true, 0, false), None)?;
    let loaded = store.load()?.active.ok_or("续局缺失")?.replay()?;
    if loaded.board != game.board || loaded.id != game.id {
        return Err("续局校验失败".into());
    }
    let engine = Engine::new();
    engine.request_game(&loaded, loaded.history.len(), 3, 1000, 2)?;
    let response = engine
        .rx
        .recv_timeout(std::time::Duration::from_secs(30))
        .map_err(|e| e.to_string())?
        .result?;
    if !loaded.board.is_legal(response.best) {
        return Err("真实引擎走法无效".into());
    }
    game.resign(Side::Red);
    let record = game.snapshot(true, 0, false);
    store.save_game(record.clone(), None)?;
    store.save_game(record, None)?;
    if store.load()?.archives.len() != 1 {
        return Err("归档幂等校验失败".into());
    }
    crate::diagnostics::record(
        "self-test",
        "real engine, history, resume and archive passed",
    );
    Ok(())
}
