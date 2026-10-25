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
    if std::env::args().nth(1).as_deref() == Some("--cold-load") {
        let store = Storage::new(std::env::args().nth(2).ok_or("missing fixture")?.into());
        let start = Instant::now();
        let data = store.load()?;
        println!(
            "{}",
            serde_json::json!({"loadMs":start.elapsed().as_secs_f64()*1000.,"archives":data.archives.len()})
        );
        return Ok(());
    }
    let output = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/performance-baseline.json".into());
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let store = Storage::new(dir.path().join("state.json"));
    let mut game = Game::default();
    let mut seed = 936137555u64;
    for _ in 0..300 {
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
    let mut preferences = Vec::new();
    let mut full_rewrites = Vec::new();
    for sample in 0..20 {
        let mut settings = data.settings.clone();
        settings.music_volume = sample as f32 / 20.;
        let start = Instant::now();
        store.save_settings(settings)?;
        preferences.push(start.elapsed().as_secs_f64() * 1000.);
        let start = Instant::now();
        store.update(|_| Ok(()))?;
        full_rewrites.push(start.elapsed().as_secs_f64() * 1000.);
    }
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
    let distinct = Storage::new(dir.path().join("distinct.json"));
    let source = data.archives.first().ok_or("missing history")?.replay()?;
    distinct.update(|d| {
        for index in 0..150 {
            let ply = (source.history.len().saturating_sub(150) + index)
                .min(source.history.len().saturating_sub(1));
            let mut game = source.branch_at(ply)?;
            game.resign(Side::Red);
            let mut record = game.snapshot(true, 0, false);
            record.favorite = index < 50;
            d.archives.push(record);
        }
        Ok(())
    })?;
    let mut cold = Vec::new();
    for _ in 0..5 {
        let result =
            std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
                .arg("--cold-load")
                .arg(&distinct.path)
                .output()
                .map_err(|e| e.to_string())?;
        if !result.status.success() {
            return Err("cold fixture process failed".into());
        }
        let value: serde_json::Value =
            serde_json::from_slice(&result.stdout).map_err(|e| e.to_string())?;
        cold.push(value["loadMs"].as_f64().ok_or("missing cold measurement")?);
    }
    let result = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,"device":std::env::var("XIANGQI_BENCH_DEVICE").unwrap_or_else(|_|"unspecified local host".into()),"debugAssertions":cfg!(debug_assertions),"assets":tst_xiangqi::persistence::asset_fingerprint()?,"seed":936137555u64,"plies":game_plies(&data),"archivesRetained":data.archives.len(),"storage150":{"p50Ms":saves[75],"p95Ms":saves[142],"maxMs":saves[149],"loadMs":load_ms,"bytes":std::fs::metadata(&store.path).map_err(|e|e.to_string())?.len()},"distinctHistoriesColdProcesses":summary(cold),"preferences":summary(preferences),"fullLibraryTransaction":summary(full_rewrites),"search":search,"diagnostics":tst_xiangqi::diagnostics::snapshot(),"scope":"local temporary data; first library shares one history; distinct library uses 150 different prefixes in five fresh processes with OS file cache; cold first engine and warm later searches; growing library saves; no ELO/device battery claim"});
    if let Some(parent) = std::path::Path::new(&output).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut f = std::fs::File::create(&output).map_err(|e| e.to_string())?;
    f.write_all(serde_json::to_string_pretty(&result).unwrap().as_bytes())
        .map_err(|e| e.to_string())?;
    println!("{output}");
    Ok(())
}
fn summary(mut samples: Vec<f64>) -> serde_json::Value {
    samples.sort_by(f64::total_cmp);
    serde_json::json!({"samples":samples.len(),"p50Ms":samples[samples.len()/2],"p95Ms":samples[samples.len()*95/100],"minMs":samples[0],"maxMs":samples[samples.len()-1],"rawMs":samples})
}
fn game_plies(d: &tst_xiangqi::storage::Data) -> usize {
    d.archives.first().map_or(0, |r| r.history.len())
}
