use tst_xiangqi::{
    game::{Game, Outcome},
    notation::chinese,
    rules::{Board, Move, START_FEN, Side},
};
fn mv(s: &str) -> Move {
    s.parse().unwrap()
}
fn perft(b: &Board, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }
    b.legal_moves()
        .into_iter()
        .map(|m| {
            let mut next = b.clone();
            next.play(m).unwrap();
            perft(&next, depth - 1)
        })
        .sum()
}
#[test]
fn standard_position_perft_and_no_mutation() {
    let board = Board::default();
    assert_eq!(perft(&board, 1), 44);
    assert_eq!(perft(&board, 2), 1920);
    assert_eq!(perft(&board, 3), 79666);
    assert_eq!(board.fen(), START_FEN);
}
#[test]
fn strict_fen_and_uci_validation() {
    assert!(tst_xiangqi::rules::Square::new(i8::MAX, i8::MAX).is_none());
    assert!(tst_xiangqi::rules::Square::new(i8::MIN, i8::MIN).is_none());
    for fen in [
        "",
        "9/9/9/9/9/9/9/9/9/9 w - - 0 1",
        "4k4/9/9/9/9/9/9/9/9/4K3 w - - 0 1",
        "4k4/99/9/9/9/9/9/9/9/4K4 w - - 0 1",
        "4k4/9/9/9/9/9/9/9/9/4K4 x - - 0 1",
        "4k4/9/9/9/9/9/9/9/9/4K4 w - - -1 1",
        "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 0",
    ] {
        assert!(fen.parse::<Board>().is_err(), "{fen}");
    }
    for s in ["a0a0", "j0a1", "a:a1", "a0a1\n", "é12", "a0a1 "] {
        assert!(s.parse::<Move>().is_err(), "{s}");
    }
    assert_eq!(mv("a0i9").to_string(), "a0i9");
}
#[test]
fn horse_leg_elephant_eye_cannon_and_flying_generals() {
    let mut b = Board::default();
    assert!(b.is_legal(mv("b0c2")));
    assert!(!b.is_legal(mv("b0d1")));
    assert!(b.is_legal(mv("c0e2")));
    assert!(b.is_legal(mv("c0a2")));
    assert!(b.is_legal(mv("b2b9")));
    assert!(!b.is_legal(mv("b2b7")));
    b.play(mv("b2e2")).unwrap();
    assert!(!b.is_legal(mv("a6a7")));
    let b: Board = "4k4/9/9/9/4P4/9/9/9/9/4K4 w - - 0 1".parse().unwrap();
    assert!(!b.is_legal(mv("e5d5")));
    assert!(b.is_legal(mv("e5e6")));
    // Facing kings are rejected at the external-input boundary.
    assert!(
        "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1"
            .parse::<Board>()
            .is_err()
    );
}
#[test]
fn undo_restores_all_counters_and_capture() {
    let mut g = Game::default();
    let start = g.board.clone();
    g.play(mv("b2b9")).unwrap();
    assert_eq!(g.board.halfmove, 0);
    g.undo_to_player(Side::Red);
    assert_eq!(g.board, start);
    g.play(mv("b0c2")).unwrap();
    g.play(mv("b9c7")).unwrap();
    assert_eq!(g.board.fullmove, 2);
    assert_eq!(g.board.halfmove, 2);
    g.undo_to_player(Side::Red);
    assert_eq!(g.board, start);
}
#[test]
fn black_first_perpetual_check_is_black_loss() {
    let board: Board = "5k3/9/9/9/9/9/9/9/3r5/4K4 b - - 0 1".parse().unwrap();
    let mut g = Game::new(board);
    for m in [
        "d1e1", "e0d0", "e1d1", "d0e0", "d1e1", "e0d0", "e1d1", "d0e0",
    ] {
        g.play(mv(m)).unwrap();
    }
    assert_eq!(g.outcome, Outcome::RedWin);
    assert_eq!(g.reason, "黑方长将判负");
}
#[test]
fn repetition_and_no_moves_terminal_rules() {
    let mut g = Game::default();
    for m in [
        "b0c2", "b9c7", "c2b0", "c7b9", "b0c2", "b9c7", "c2b0", "c7b9",
    ] {
        g.play(mv(m)).unwrap();
    }
    assert_eq!(g.outcome, Outcome::Draw);
    assert!(g.play(mv("a3a4")).is_err());
    let b: Board = "3k5/2R1R4/9/9/9/9/9/9/9/4K4 b - - 0 1".parse().unwrap();
    assert!(!b.in_check(Side::Black));
    assert_eq!(Game::new(b).outcome, Outcome::RedWin);
    let b: Board = START_FEN.replace("0 1", "120 80").parse().unwrap();
    assert_eq!(Game::new(b).outcome, Outcome::Draw);
}
#[test]
fn notation_and_legacy_replay() {
    let b = Board::default();
    assert_eq!(chinese(&b, mv("h2e2")).unwrap(), "炮二平五");
    let mut b = b;
    b.play(mv("h2e2")).unwrap();
    assert_eq!(chinese(&b, mv("h9g7")).unwrap(), "马8进7");
    let json = r#"{"time":1,"userRed":true,"result":"draw","history":[{"uci":"h2e2","fen":"corrupt","notation":"incorrect"}]}"#;
    let record: tst_xiangqi::game::Record = serde_json::from_str(json).unwrap();
    let game = record.replay().unwrap();
    assert_eq!(game.history[0].notation, "炮二平五");
}

#[test]
fn score_settlement_and_five_quality_levels_keep_legacy_thresholds() {
    use tst_xiangqi::game::{move_quality, outcome_by_score};
    assert_eq!(outcome_by_score(500), Outcome::Draw);
    assert_eq!(outcome_by_score(-500), Outcome::Draw);
    assert_eq!(outcome_by_score(501), Outcome::RedWin);
    assert_eq!(outcome_by_score(-501), Outcome::BlackWin);
    assert_eq!(move_quality(0, false), "优秀");
    assert_eq!(move_quality(30, false), "良好");
    assert_eq!(move_quality(100, false), "平庸");
    assert_eq!(move_quality(250, false), "较差");
    assert_eq!(move_quality(600, false), "错招");
    assert_eq!(move_quality(1000, true), "优秀");
}
