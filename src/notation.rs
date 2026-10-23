use crate::rules::{Board, Kind, Move, Side};
const CN: [&str; 9] = ["一", "二", "三", "四", "五", "六", "七", "八", "九"];
fn number(n: usize, side: Side) -> String {
    if side == Side::Red {
        CN.get(n.saturating_sub(1)).unwrap_or(&"?").to_string()
    } else {
        n.to_string()
    }
}
fn file(f: i8, side: Side) -> String {
    number(
        if side == Side::Red {
            9 - f as usize
        } else {
            f as usize + 1
        },
        side,
    )
}
pub fn chinese(board: &Board, m: Move) -> Result<String, String> {
    let p = board.at(m.from).ok_or("记谱起点无棋子")?;
    let mut origin = file(m.from.file(), p.side);
    let mut prefix = String::new();
    if !matches!(p.kind, Kind::Advisor | Kind::Elephant) {
        let mut groups: Vec<Vec<_>> = (0..9)
            .map(|f| {
                board
                    .pieces()
                    .filter(|(s, q)| q == &p && s.file() == f)
                    .map(|(s, _)| s)
                    .collect()
            })
            .collect();
        for group in &mut groups {
            group.sort_by_key(|s| {
                if p.side == Side::Red {
                    s.rank()
                } else {
                    -s.rank()
                }
            });
        }
        let multi = groups.iter().filter(|g| g.len() >= 2).count();
        let group = &groups[m.from.file() as usize];
        if p.kind == Kind::Pawn && multi >= 2 && group.len() >= 2 {
            let mut fs: Vec<_> = (0..9).collect();
            if p.side == Side::Red {
                fs.reverse();
            }
            let all: Vec<_> = fs
                .into_iter()
                .filter(|&f| groups[f].len() >= 2)
                .flat_map(|f| groups[f].iter().copied())
                .collect();
            prefix = number(all.iter().position(|&s| s == m.from).unwrap() + 1, p.side);
            origin.clear();
        } else if group.len() >= 2 {
            let index = group.iter().position(|&s| s == m.from).unwrap();
            prefix = match group.len() {
                2 => ["前", "后"][index].into(),
                3 => ["前", "中", "后"][index].into(),
                _ => number(index + 1, p.side),
            };
            origin.clear();
        }
    }
    let dr = m.to.rank() - m.from.rank();
    let action = if dr == 0 {
        "平"
    } else if dr.signum() == p.side.forward() {
        "进"
    } else {
        "退"
    };
    let dest = if m.from.file() == m.to.file() {
        number(dr.unsigned_abs() as usize, p.side)
    } else {
        file(m.to.file(), p.side)
    };
    Ok(format!("{prefix}{}{origin}{action}{dest}", p.label()))
}
