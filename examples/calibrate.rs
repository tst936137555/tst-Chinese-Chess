//! Fixed-seed difficulty calibration against deeper engine evaluations; this is not an ELO estimate.
use std::{collections::BTreeMap, time::Duration};
use tst_xiangqi::{
    engine::{Analysis, Engine, Score, choose_variation, score_loss},
    game::{Game, LEVELS, Outcome},
    review::rule_score,
    rules::Side,
};
fn search(
    engine: &Engine,
    game: &Game,
    depth: u32,
    ms: u64,
    pv: usize,
) -> Result<Analysis, String> {
    let mut cold = game.clone();
    cold.fork_identity();
    engine.request_game(&cold, cold.history.len(), depth, ms, pv)?;
    engine
        .rx
        .recv_timeout(Duration::from_secs(40))
        .map_err(|e| e.to_string())?
        .result
}
fn main() -> Result<(), String> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/calibration.json".into());
    let engine = Engine::new();
    let mut mid = Game::default();
    let mut seed = 936137555u64;
    for _ in 0..80 {
        if mid.outcome != Outcome::Playing {
            break;
        }
        let moves = mid.board.legal_moves();
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        mid.play(moves[seed as usize % moves.len()])?;
    }
    let mut rows = Vec::new();
    for (case, game) in [("opening", Game::default()), ("midgame", mid)] {
        let reference = search(&engine, &game, 16, 1500, 1)?;
        let ref_score = reference.variations[0].evaluation;
        let mut evaluated = BTreeMap::new();
        for level in LEVELS {
            let a = search(&engine, &game, level.depth, level.ms, level.pv)?;
            let mut counts = BTreeMap::<String, usize>::new();
            for draw in 0..1024 {
                let mv = choose_variation(
                    &a,
                    game.board.turn,
                    level.tolerance,
                    936137555 + draw,
                    game.history.len(),
                );
                *counts.entry(mv.to_string()).or_default() += 1;
            }
            let mut weighted_loss = 0f64;
            let mut known = 0usize;
            for (mv, count) in &counts {
                let score = if let Some(score) = evaluated.get(mv) {
                    *score
                } else {
                    let mut child = game.clone();
                    child.play(mv.parse()?)?;
                    let score = if let Some(score) = rule_score(&child, child.history.len()) {
                        score
                    } else {
                        search(&engine, &child, 14, 1000, 1)?.variations[0].evaluation
                    };
                    evaluated.insert(mv.clone(), score);
                    score
                };
                if matches!((ref_score, score), (Score::Cp(_), Score::Cp(_)))
                    && let Some(loss) = score_loss(ref_score, score, game.board.turn)
                {
                    weighted_loss += loss as f64 * *count as f64;
                    known += count;
                }
            }
            rows.push(serde_json::json!({"case":case,"side":if game.board.turn==Side::Red{"red"}else{"black"},"level":level.name,"referenceDepth":reference.variations[0].depth,"referenceBest":reference.best.to_string(),"actualDepth":a.variations[0].depth,"searchMs":a.elapsed_ms,"samples":1024,"choices":counts,"estimatedMeanCpLoss":if known>0{Some(weighted_loss/known as f64)}else{None},"knownCpSamples":known}));
        }
    }
    let output = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,"seed":936137555u64,"scope":"two fixed positions; deeper-search reference; horizon noise remains; no human skill or ELO claim","rows":rows});
    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, serde_json::to_vec_pretty(&output).unwrap())
        .map_err(|e| e.to_string())?;
    println!("{path}");
    Ok(())
}
