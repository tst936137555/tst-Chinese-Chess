//! Pure review scheduling: the viewer cursor never follows background progress.
use crate::{engine::Score, game::Game};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub fn rule_score(game: &Game, ply: usize) -> Option<Score> {
    use crate::game::{EndReason, Outcome};
    if ply != game.history.len()
        || matches!(
            game.end_reason,
            None | Some(
                EndReason::Resignation { .. } | EndReason::Evaluation { .. } | EndReason::Legacy
            )
        )
    {
        return None;
    }
    match game.outcome {
        Outcome::RedWin => Some(Score::Terminal(1)),
        Outcome::BlackWin => Some(Score::Terminal(-1)),
        Outcome::Draw => Some(Score::Cp(0)),
        Outcome::Playing => None,
    }
}
pub const ANALYSIS_LIMITS: (u32, u64) = (12, 3000);
#[derive(Clone, Serialize, Deserialize)]
pub struct CachedPosition {
    pub score: Score,
    pub best: Option<String>,
    pub depth: u32,
}
impl CachedPosition {
    pub fn valid_for(&self, game: &Game, ply: usize) -> bool {
        if !self.score.is_valid() || self.depth > 64 {
            return false;
        }
        if let Some(score) = rule_score(game, ply) {
            return self.score == score && self.best.is_none();
        }
        !matches!(self.score, Score::Terminal(_))
            && self.depth > 0
            && self
                .best
                .as_ref()
                .and_then(|s| s.parse().ok())
                .is_some_and(|m| game.at_ply(ply).is_some_and(|b| b.is_legal(m)))
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct CachedReview {
    pub entries: Vec<Option<CachedPosition>>,
    pub updated_at: u64,
}
pub fn key(game: &Game) -> String {
    let mut hash = Sha256::new();
    hash.update(b"review-v2\0");
    hash.update(game.start.fen());
    for e in &game.history {
        hash.update(e.mv.to_string());
    }
    // Preserve existing full-analysis cache identities.
    hash.update(b"Deep");
    format!("{:x}", hash.finalize())
}
pub fn next_missing(cursor: usize, known: &[bool]) -> Option<usize> {
    [Some(cursor), cursor.checked_sub(1), cursor.checked_add(1)]
        .into_iter()
        .flatten()
        .find(|&i| known.get(i) == Some(&false))
        .or_else(|| known.iter().position(|done| !done))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_priority_and_history_sensitive_keys() {
        assert_eq!(
            next_missing(3, &[false, false, false, false, false]),
            Some(3)
        );
        assert_eq!(
            next_missing(3, &[false, false, false, true, false]),
            Some(2)
        );
        assert_eq!(next_missing(0, &[true, true]), None);
        let g = Game::default();
        let mut h = g.clone();
        for m in ["b0c2", "b9c7", "c2b0", "c7b9"] {
            h.play(m.parse().unwrap()).unwrap();
        }
        assert_eq!(g.board.position_key(), h.board.position_key());
        assert_ne!(key(&g), key(&h));
    }
}
