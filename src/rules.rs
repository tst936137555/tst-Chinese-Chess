//! Coordinates: file 0..8, rank 0..9 (black's home rank is zero).
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

pub const START_FEN: &str = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Red,
    Black,
}
impl Side {
    pub fn other(self) -> Self {
        match self {
            Self::Red => Self::Black,
            Self::Black => Self::Red,
        }
    }
    pub fn forward(self) -> i8 {
        if self == Self::Red { -1 } else { 1 }
    }
    pub fn label(self) -> &'static str {
        if self == Self::Red {
            "红方"
        } else {
            "黑方"
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    King,
    Advisor,
    Elephant,
    Horse,
    Rook,
    Cannon,
    Pawn,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Piece {
    pub side: Side,
    pub kind: Kind,
}
impl Piece {
    pub fn letter(self) -> char {
        let c = match self.kind {
            Kind::King => 'k',
            Kind::Advisor => 'a',
            Kind::Elephant => 'b',
            Kind::Horse => 'n',
            Kind::Rook => 'r',
            Kind::Cannon => 'c',
            Kind::Pawn => 'p',
        };
        if self.side == Side::Red {
            c.to_ascii_uppercase()
        } else {
            c
        }
    }
    pub fn label(self) -> &'static str {
        match (self.side, self.kind) {
            (Side::Red, Kind::King) => "帅",
            (_, Kind::King) => "将",
            (Side::Red, Kind::Advisor) => "仕",
            (_, Kind::Advisor) => "士",
            (Side::Red, Kind::Elephant) => "相",
            (_, Kind::Elephant) => "象",
            (_, Kind::Horse) => "马",
            (_, Kind::Rook) => "车",
            (_, Kind::Cannon) => "炮",
            (Side::Red, Kind::Pawn) => "兵",
            (_, Kind::Pawn) => "卒",
        }
    }
    fn parse(c: char) -> Result<Self, String> {
        let kind = match c.to_ascii_lowercase() {
            'k' => Kind::King,
            'a' => Kind::Advisor,
            'b' | 'e' => Kind::Elephant,
            'n' | 'h' => Kind::Horse,
            'r' => Kind::Rook,
            'c' => Kind::Cannon,
            'p' => Kind::Pawn,
            _ => return Err(format!("非法棋子：{c}")),
        };
        Ok(Self {
            side: if c.is_ascii_uppercase() {
                Side::Red
            } else {
                Side::Black
            },
            kind,
        })
    }
}
/// The private index makes out-of-board squares unrepresentable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Square(u8);
impl Square {
    pub fn new(file: i8, rank: i8) -> Option<Self> {
        if (0..9).contains(&file) && (0..10).contains(&rank) {
            Some(Self((rank * 9 + file) as u8))
        } else {
            None
        }
    }
    pub fn file(self) -> i8 {
        (self.0 % 9) as i8
    }
    pub fn rank(self) -> i8 {
        (self.0 / 9) as i8
    }
    fn offset(self, df: i8, dr: i8) -> Option<Self> {
        Self::new(self.file() + df, self.rank() + dr)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub from: Square,
    pub to: Square,
}
impl FromStr for Move {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let b = s.as_bytes();
        if b.len() != 4
            || !(b'a'..=b'i').contains(&b[0])
            || !b[1].is_ascii_digit()
            || !(b'a'..=b'i').contains(&b[2])
            || !b[3].is_ascii_digit()
        {
            return Err(format!("无效 UCI 走法：{s}"));
        }
        let from = Square::new((b[0] - b'a') as i8, 9 - (b[1] - b'0') as i8).unwrap();
        let to = Square::new((b[2] - b'a') as i8, 9 - (b[3] - b'0') as i8).unwrap();
        if from == to {
            return Err("走法起终点相同".into());
        }
        Ok(Self { from, to })
    }
}
impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{}{}",
            (b'a' + self.from.file() as u8) as char,
            9 - self.from.rank(),
            (b'a' + self.to.file() as u8) as char,
            9 - self.to.rank()
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    squares: [Option<Piece>; 90],
    pub turn: Side,
    pub halfmove: u32,
    pub fullmove: u32,
}
impl Default for Board {
    fn default() -> Self {
        START_FEN.parse().expect("constant FEN")
    }
}
impl FromStr for Board {
    type Err = String;
    fn from_str(fen: &str) -> Result<Self, Self::Err> {
        if fen.len() > 256 {
            return Err("FEN 过长".into());
        }
        let parts: Vec<_> = fen.split_whitespace().collect();
        if !matches!(parts.len(), 2 | 6) {
            return Err("FEN 必须有 2 或 6 个字段".into());
        }
        let turn = match parts[1] {
            "w" | "r" => Side::Red,
            "b" => Side::Black,
            _ => return Err("FEN 行棋方无效".into()),
        };
        let mut b = Self {
            squares: [None; 90],
            turn,
            halfmove: 0,
            fullmove: 1,
        };
        let ranks: Vec<_> = parts[0].split('/').collect();
        if ranks.len() != 10 {
            return Err("FEN 必须有 10 行".into());
        }
        for (r, row) in ranks.iter().enumerate() {
            let mut file = 0usize;
            for c in row.chars() {
                if ('1'..='9').contains(&c) {
                    file += c.to_digit(10).unwrap() as usize;
                } else {
                    if file >= 9 {
                        return Err("FEN 行超过 9 格".into());
                    }
                    b.squares[r * 9 + file] = Some(Piece::parse(c)?);
                    file += 1;
                }
                if file > 9 {
                    return Err("FEN 行超过 9 格".into());
                }
            }
            if file != 9 {
                return Err("FEN 每行必须恰好 9 格".into());
            }
        }
        for side in [Side::Red, Side::Black] {
            let kings: Vec<_> = b
                .pieces()
                .filter(|(_, p)| p.side == side && p.kind == Kind::King)
                .collect();
            if kings.len() != 1 || !palace(kings[0].0, side) {
                return Err("每方必须恰有一个位于九宫内的将帅".into());
            }
        }
        b.validate_pieces()?;
        if parts.len() == 6 {
            if parts[2] != "-" || parts[3] != "-" {
                return Err("象棋 FEN 第三、四字段应为 -".into());
            }
            b.halfmove = parts[4].parse().map_err(|_| "FEN 半回合数无效")?;
            b.fullmove = parts[5].parse().map_err(|_| "FEN 回合数无效")?;
            if b.fullmove == 0 {
                return Err("FEN 回合数必须大于零".into());
            }
        }
        Ok(b)
    }
}
fn palace(s: Square, side: Side) -> bool {
    (3..=5).contains(&s.file())
        && if side == Side::Red {
            s.rank() >= 7
        } else {
            s.rank() <= 2
        }
}
impl Board {
    fn validate_pieces(&self) -> Result<(), String> {
        for side in [Side::Red, Side::Black] {
            for (kind, limit) in [
                (Kind::King, 1),
                (Kind::Advisor, 2),
                (Kind::Elephant, 2),
                (Kind::Horse, 2),
                (Kind::Rook, 2),
                (Kind::Cannon, 2),
                (Kind::Pawn, 5),
            ] {
                if self
                    .pieces()
                    .filter(|(_, p)| p.side == side && p.kind == kind)
                    .count()
                    > limit
                {
                    return Err(format!("{}棋子数量超限", side.label()));
                }
            }
        }
        for (s, p) in self.pieces() {
            let (f, r) = if p.side == Side::Red {
                (s.file(), 9 - s.rank())
            } else {
                (s.file(), s.rank())
            };
            let valid = match p.kind {
                Kind::Advisor => matches!((f, r), (3, 0) | (5, 0) | (4, 1) | (3, 2) | (5, 2)),
                Kind::Elephant => matches!(
                    (f, r),
                    (2, 0) | (6, 0) | (0, 2) | (4, 2) | (8, 2) | (2, 4) | (6, 4)
                ),
                Kind::Pawn => r >= 3 && (r >= 5 || f % 2 == 0),
                _ => true,
            };
            if !valid {
                return Err(format!("{}{}位置无效", p.side.label(), p.label()));
            }
        }
        // Study positions may have the side to move in check, but never the other king.
        if self.in_check(self.turn.other()) {
            return Err("未行棋方将帅处于被将状态，局面无效".into());
        }
        Ok(())
    }
    pub fn at(&self, s: Square) -> Option<Piece> {
        self.squares[s.0 as usize]
    }
    pub fn pieces(&self) -> impl Iterator<Item = (Square, Piece)> + '_ {
        self.squares
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.map(|p| (Square(i as u8), p)))
    }
    pub fn position_key(&self) -> String {
        let f = self.fen();
        f.split_whitespace().take(2).collect::<Vec<_>>().join(" ")
    }
    pub fn fen(&self) -> String {
        let mut s = String::new();
        for r in 0..10 {
            let mut n = 0u8;
            for f in 0..9 {
                if let Some(p) = self.squares[r * 9 + f] {
                    if n > 0 {
                        s.push((b'0' + n) as char);
                        n = 0;
                    }
                    s.push(p.letter());
                } else {
                    n += 1;
                }
            }
            if n > 0 {
                s.push((b'0' + n) as char);
            }
            if r < 9 {
                s.push('/');
            }
        }
        format!(
            "{s} {} - - {} {}",
            if self.turn == Side::Red { "w" } else { "b" },
            self.halfmove,
            self.fullmove
        )
    }
    fn blockers(&self, from: Square, to: Square) -> Option<usize> {
        if from == to || (from.file() != to.file() && from.rank() != to.rank()) {
            return None;
        }
        let df = (to.file() - from.file()).signum();
        let dr = (to.rank() - from.rank()).signum();
        let mut at = from.offset(df, dr)?;
        let mut count = 0;
        while at != to {
            if self.at(at).is_some() {
                count += 1;
            }
            at = at.offset(df, dr)?;
        }
        Some(count)
    }
    fn attacks(&self, p: Piece, from: Square, to: Square) -> bool {
        let df = to.file() - from.file();
        let dr = to.rank() - from.rank();
        let af = df.abs();
        let ar = dr.abs();
        if from == to {
            return false;
        }
        match p.kind {
            Kind::King => {
                (af + ar == 1 && palace(to, p.side))
                    || (df == 0
                        && self
                            .at(to)
                            .is_some_and(|q| q.kind == Kind::King && q.side != p.side)
                        && self.blockers(from, to) == Some(0))
            }
            Kind::Advisor => af == 1 && ar == 1 && palace(to, p.side),
            Kind::Elephant => {
                af == 2
                    && ar == 2
                    && (if p.side == Side::Red {
                        to.rank() >= 5
                    } else {
                        to.rank() <= 4
                    })
                    && from
                        .offset(df / 2, dr / 2)
                        .is_some_and(|s| self.at(s).is_none())
            }
            Kind::Horse => {
                ((af == 2 && ar == 1) || (af == 1 && ar == 2))
                    && from
                        .offset(
                            if af == 2 { df / 2 } else { 0 },
                            if ar == 2 { dr / 2 } else { 0 },
                        )
                        .is_some_and(|s| self.at(s).is_none())
            }
            Kind::Rook => self.blockers(from, to) == Some(0),
            Kind::Cannon => self.blockers(from, to) == Some(usize::from(self.at(to).is_some())),
            Kind::Pawn => {
                (df == 0 && dr == p.side.forward())
                    || (ar == 0
                        && af == 1
                        && if p.side == Side::Red {
                            from.rank() <= 4
                        } else {
                            from.rank() >= 5
                        })
            }
        }
    }
    pub fn in_check(&self, side: Side) -> bool {
        let king = self
            .pieces()
            .find(|(_, p)| p.side == side && p.kind == Kind::King)
            .map(|(s, _)| s);
        let Some(king) = king else {
            return true;
        };
        self.pieces()
            .any(|(s, p)| p.side != side && self.attacks(p, s, king))
    }
    pub fn is_legal(&self, m: Move) -> bool {
        let Some(p) = self.at(m.from) else {
            return false;
        };
        if p.side != self.turn
            || self
                .at(m.to)
                .is_some_and(|q| q.side == p.side || q.kind == Kind::King)
            || !self.attacks(p, m.from, m.to)
        {
            return false;
        }
        let mut next = self.clone();
        next.apply_unchecked(m);
        !next.in_check(p.side)
    }
    pub fn has_legal_move(&self) -> bool {
        self.pieces().any(|(from, p)| {
            p.side == self.turn
                && (0..90).any(|i| {
                    self.is_legal(Move {
                        from,
                        to: Square(i),
                    })
                })
        })
    }
    pub fn legal_moves(&self) -> Vec<Move> {
        let mut moves = Vec::new();
        for (from, p) in self.pieces() {
            if p.side != self.turn {
                continue;
            }
            for i in 0..90 {
                let m = Move {
                    from,
                    to: Square(i),
                };
                if self.is_legal(m) {
                    moves.push(m);
                }
            }
        }
        moves
    }
    pub fn play(&mut self, m: Move) -> Result<Option<Piece>, String> {
        if !self.is_legal(m) {
            return Err(format!("非法走法：{m}"));
        }
        Ok(self.apply_unchecked(m))
    }
    fn apply_unchecked(&mut self, m: Move) -> Option<Piece> {
        let captured = self.at(m.to);
        self.squares[m.to.0 as usize] = self.at(m.from);
        self.squares[m.from.0 as usize] = None;
        self.halfmove = if captured.is_some() {
            0
        } else {
            self.halfmove.saturating_add(1)
        };
        if self.turn == Side::Black {
            self.fullmove = self.fullmove.saturating_add(1);
        }
        self.turn = self.turn.other();
        captured
    }
}
