//! Fixed-seed difficulty calibration against deeper engine evaluations; this is not an ELO estimate.
use std::{collections::BTreeMap, time::Duration};
use tst_xiangqi::{
    engine::{Analysis, Engine, Score, choose_variation, score_loss},
    game::{Game, LEVELS},
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
    let args: Vec<_> = std::env::args().collect();
    let option = |name, default: usize| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default)
    };
    let max_plies = option("--max-plies", 512).clamp(1, 4096);
    let pairs = option("--pairs", 1).clamp(1, 20);
    let mut rows = Vec::new();
    for (case, game) in tst_xiangqi::calibration::positions() {
        eprintln!("Calibration: {case}");
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
    let mut matches = Vec::new();
    if std::env::args().any(|arg| arg == "--matches") {
        for lower in 0..4 {
            for pair in 0..pairs {
                for reverse in [false, true] {
                    let mut game = Game::default();
                    let mut played = 0;
                    while game.outcome == tst_xiangqi::game::Outcome::Playing && played < max_plies
                    {
                        let high = (game.board.turn == Side::Red) == reverse;
                        let index = lower + usize::from(high);
                        let level = LEVELS[index];
                        let analysis = search(&engine, &game, level.depth, level.ms, level.pv)?;
                        let mv = choose_variation(
                            &analysis,
                            game.board.turn,
                            level.tolerance,
                            936137555 + lower as u64 + 1009 * pair as u64,
                            played,
                        );
                        game.play(mv)?;
                        played += 1;
                    }
                    eprintln!(
                        "Paired game: {} vs {}, reverse={reverse}, {played} plies",
                        LEVELS[lower].name,
                        LEVELS[lower + 1].name
                    );
                    matches.push(serde_json::json!({"pair":pair,"lowerIndex":lower,"lower":LEVELS[lower].name,"higher":LEVELS[lower+1].name,"higherIsRed":reverse,"plies":played,"result":game.outcome,"history":game.history.iter().map(|e|e.mv.to_string()).collect::<Vec<_>>() }));
                }
            }
        }
    }
    let summaries:Vec<_>=(0..4).map(|lower| {
        let games:Vec<_>=matches.iter().filter(|g|g["lowerIndex"]==lower).collect();
        let mut wins=0;let mut losses=0;let mut draws=0;let mut unfinished=0;
        for g in &games { match g["result"].as_str() {
            Some("redWin")|Some("blackWin") => { let red=g["result"]=="redWin";if red==g["higherIsRed"].as_bool().unwrap() {wins+=1;}else{losses+=1;} },
            Some("draw")=>draws+=1,_=>unfinished+=1
        } }
        serde_json::json!({"lower":LEVELS[lower as usize].name,"higher":LEVELS[lower as usize+1].name,"wins":wins,"losses":losses,"draws":draws,"unfinished":unfinished,"decisiveWin95Percent":tst_xiangqi::calibration::decisive_win_interval(wins,wins+losses)})
    }).collect();
    let output = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,"assets":tst_xiangqi::persistence::asset_fingerprint()?,"fixtureVersion":2,"seed":936137555u64,"maxPlies":max_plies,"pairsPerAdjacentLevel":pairs,"scope":"sixteen versioned positions, both colors; deeper-search reference; paired games run to rule ending or explicit limit; unfinished excluded from win rates; Wilson interval uses decisive games only; no human skill or ELO claim","rows":rows,"pairedGames":matches,"pairedSummary":summaries});
    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, serde_json::to_vec_pretty(&output).unwrap())
        .map_err(|e| e.to_string())?;
    println!("{path}");
    Ok(())
}
