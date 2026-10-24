use serde::Deserialize;
use tst_xiangqi::rules::Board;
#[derive(Deserialize)]
struct Case {
    name: String,
    fen: String,
    perft1: usize,
    perft2: usize,
}
#[test]
fn pinned_pikafish_corpus_covers_midgames_checks_pins_cannons_and_horses() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("rule_corpus.json")).unwrap();
    assert!(cases.len() >= 19);
    for case in cases {
        let board: Board = case.fen.parse().unwrap();
        let moves = board.legal_moves();
        assert_eq!(moves.len(), case.perft1, "{}", case.name);
        let nodes: usize = moves
            .into_iter()
            .map(|mv| {
                let mut next = board.clone();
                next.play(mv).unwrap();
                next.legal_moves().len()
            })
            .sum();
        assert_eq!(nodes, case.perft2, "{}", case.name);
        assert_eq!(board.fen(), case.fen);
    }
}
#[test]
fn calibration_confidence_excludes_empty_samples_and_bounds_extremes() {
    use tst_xiangqi::calibration::decisive_win_interval;
    assert_eq!(decisive_win_interval(0, 0), None);
    let low = decisive_win_interval(0, 20).unwrap();
    let high = decisive_win_interval(20, 20).unwrap();
    assert!(low.1 < 0.2 && high.0 > 0.8);
    let middle = decisive_win_interval(10, 20).unwrap();
    assert!(middle.0 < 0.5 && middle.1 > 0.5);
}
