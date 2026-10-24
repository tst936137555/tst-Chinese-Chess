//! Machine-readable root move counts for a separate Pikafish differential oracle.
use tst_xiangqi::{
    calibration,
    game::{Game, Outcome},
    rules::Board,
};
fn main() {
    let mut cases = calibration::positions();
    for (name, fen) in [
        ("rook-pin", "4k4/4r4/9/9/9/9/9/9/4R4/4K4 w - - 0 1"),
        ("cannon-screen", "3k5/9/9/9/4p4/9/9/9/4C4/4K4 w - - 0 1"),
        ("stalemate", "3k5/2R1R4/9/9/9/9/9/9/9/4K4 b - - 0 1"),
    ] {
        cases.push((name.into(), Game::new(fen.parse().unwrap())));
    }
    for initial in [7_u64, 42, 936137555, 0x12345678] {
        let mut game = Game::default();
        let mut seed = initial;
        for ply in 0..80 {
            if game.outcome != Outcome::Playing {
                break;
            }
            let moves = game.board.legal_moves();
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            game.play(moves[seed as usize % moves.len()]).unwrap();
            if ply % 10 == 9 {
                cases.push((format!("random-{initial}-{}", ply + 1), game.clone()));
            }
        }
    }
    let rows:Vec<_>=cases.iter().map(|(name,game)|{
        let board:&Board=&game.board;
        let roots:std::collections::BTreeMap<_,_>=board.legal_moves().into_iter().map(|mv|{let mut child=board.clone();child.play(mv).unwrap();(mv.to_string(),child.legal_moves().len())}).collect();
        serde_json::json!({"name":name,"fen":board.fen(),"perft1":roots.len(),"perft2":roots.values().sum::<usize>(),"roots":roots})
    }).collect();
    println!("{}", serde_json::to_string(&rows).unwrap());
}
