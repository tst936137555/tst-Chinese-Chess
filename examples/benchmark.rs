//! Release-only reproducible storage and five-level search baseline; no user saves.
use std::{
    io::Write,
    time::{Duration, Instant},
};
use tst_xiangqi::{
    engine::Engine,
    game::{Game, LEVELS, Outcome},
    rules::Side,
    storage::Storage,
};
fn main() -> Result<(), String> {
    let output = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/performance-baseline.json".into());
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let store = Storage::new(dir.path().join("state.json"));
    let mut game = Game::default();
    let mut seed = 936137555u64;
    for _ in 0..100 {
        if game.outcome != Outcome::Playing {
            break;
        }
        let moves = game.board.legal_moves();
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        game.play(moves[seed as usize % moves.len()])?;
    }
    let mut saves = Vec::new();
    for index in 0..150 {
        let mut g = game.clone();
        g.fork_identity();
        g.resign(Side::Red);
        let t = Instant::now();
        let mut record = g.snapshot(true, 0, false);
        record.favorite = index < 50;
        store.save_game(record, None)?;
        saves.push(t.elapsed().as_secs_f64() * 1000.);
    }
    let start = Instant::now();
    let data = store.load()?;
    let load_ms = start.elapsed().as_secs_f64() * 1000.;
    saves.sort_by(f64::total_cmp);
    let engine = Engine::new();
    let mut search = Vec::new();
    for (case, game) in [("opening", Game::default()), ("midgame", game)] {
        for level in LEVELS {
            let t = Instant::now();
            let mut case_game = game.clone();
            case_game.fork_identity();
            engine.request_game(
                &case_game,
                case_game.history.len(),
                level.depth,
                level.ms,
                level.pv,
            )?;
            let a = engine
                .rx
                .recv_timeout(Duration::from_secs(40))
                .map_err(|e| e.to_string())?
                .result?;
            search.push(serde_json::json!({"case":case,"level":level.name,"requestedDepth":level.depth,"actualDepth":a.variations.first().map(|v|v.depth),"wallMs":t.elapsed().as_secs_f64()*1000.,"searchMs":a.elapsed_ms,"budgetMs":level.ms,"uniqueCandidates":a.variations.len(),"tolerance":level.tolerance,"best":a.best.to_string(),"legal":game.board.is_legal(a.best)}));
        }
    }
    let result = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,"seed":936137555u64,"plies":game_plies(&data),"archivesRetained":data.archives.len(),"storage150":{"p50Ms":saves[75],"p95Ms":saves[142],"maxMs":saves[149],"loadMs":load_ms,"bytes":std::fs::metadata(&store.path).map_err(|e|e.to_string())?.len()},"search":search,"diagnostics":tst_xiangqi::diagnostics::snapshot(),"scope":"desktop release storage and search; not ELO, device memory or battery benchmark"});
    if let Some(parent) = std::path::Path::new(&output).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut f = std::fs::File::create(&output).map_err(|e| e.to_string())?;
    f.write_all(serde_json::to_string_pretty(&result).unwrap().as_bytes())
        .map_err(|e| e.to_string())?;
    println!("{output}");
    Ok(())
}
fn game_plies(d: &tst_xiangqi::storage::Data) -> usize {
    d.archives.first().map_or(0, |r| r.history.len())
}
