//! Fixed, versioned positions shared by calibration and rule/engine regression checks.
use crate::{
    game::Game,
    rules::{Board, Side},
};

pub fn positions() -> Vec<(String, Game)> {
    let mut middle = Game::default();
    for mv in [
        "h2e2", "h9g7", "h0g2", "b9c7", "i0h0", "i9h9", "b0c2", "a6a5",
    ] {
        middle
            .play(mv.parse().expect("fixture move"))
            .expect("legal fixture");
    }
    let fixtures = [
        ("opening", Board::default()),
        ("middle", middle.board),
        (
            "attack",
            "5k3/9/9/9/9/9/9/9/3R5/4K4 w - - 0 1"
                .parse()
                .expect("attack fixture"),
        ),
        (
            "defense",
            "5k3/9/9/9/9/9/9/9/4r4/4K4 w - - 0 1"
                .parse()
                .expect("defense fixture"),
        ),
        (
            "ending",
            "4k4/9/9/4p4/4P4/9/9/9/9/4K4 w - - 0 1"
                .parse()
                .expect("ending fixture"),
        ),
        (
            "pin",
            "4k4/4r4/9/9/9/9/9/9/4R4/4K4 w - - 0 1"
                .parse()
                .expect("pin fixture"),
        ),
        (
            "cannon",
            "3k5/9/9/9/4p4/9/9/9/4C4/4K4 w - - 0 1"
                .parse()
                .expect("cannon fixture"),
        ),
        (
            "horse",
            "3k5/9/9/9/9/9/9/9/1R7/1N3K3 w - - 0 1"
                .parse()
                .expect("horse fixture"),
        ),
    ];
    fixtures
        .into_iter()
        .flat_map(|(name, board)| {
            let fen = board.fen();
            let ranks = fen
                .split_whitespace()
                .next()
                .unwrap()
                .split('/')
                .rev()
                .map(|row| {
                    row.chars()
                        .rev()
                        .map(|c| {
                            if c.is_ascii_uppercase() {
                                c.to_ascii_lowercase()
                            } else {
                                c.to_ascii_uppercase()
                            }
                        })
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("/");
            let flipped: Board = format!(
                "{ranks} {} - - {} {}",
                if board.turn == Side::Red { "b" } else { "w" },
                board.halfmove,
                board.fullmove
            )
            .parse()
            .expect("reflected fixture");
            [
                (format!("{name}-red"), Game::new(board)),
                (format!("{name}-black"), Game::new(flipped)),
            ]
        })
        .collect()
}

/// 95% Wilson interval for wins among decisive games; draws are reported separately.
pub fn decisive_win_interval(wins: usize, games: usize) -> Option<(f64, f64)> {
    if games == 0 || wins > games {
        return None;
    }
    let n = games as f64;
    let p = wins as f64 / n;
    let z2 = 1.96_f64.powi(2);
    let center = (p + z2 / (2. * n)) / (1. + z2 / n);
    let radius = 1.96 * (p * (1. - p) / n + z2 / (4. * n * n)).sqrt() / (1. + z2 / n);
    Some(((center - radius).max(0.), (center + radius).min(1.)))
}
