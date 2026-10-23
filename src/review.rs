//! Pure review scheduling: the viewer cursor never follows background progress.
use crate::{engine::Score, game::Game};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub type ChartCache = (u64, usize, Vec<(usize, Option<Score>)>);
pub struct ReviewSession {
    pub game: Game,
    pub ply: usize,
    pub scores: Vec<Option<Score>>,
    pub depths: Vec<u32>,
    pub cache_key: String,
    pub cache_loading: bool,
    pub bests: Vec<Option<crate::rules::Move>>,
    pub analyzing: bool,
    pub user_red: bool,
    pub score_version: u64,
    pub chart_cache: ChartCache,
}
impl ReviewSession {
    pub fn new(game: Game, user_red: bool) -> Self {
        let n = game.history.len() + 1;
        Self {
            cache_key: key(&game),
            game,
            ply: 0,
            scores: vec![None; n],
            depths: vec![0; n],
            cache_loading: true,
            bests: vec![None; n],
            analyzing: true,
            user_red,
            score_version: 0,
            chart_cache: (u64::MAX, 0, Vec::new()),
        }
    }
    pub fn cache_loaded(&mut self, cache: Option<CachedReview>) {
        self.cache_loading = false;
        if let Some(cache) = cache.filter(|c| c.entries.len() == self.scores.len()) {
            for (ply, entry) in cache.entries.into_iter().enumerate() {
                if let Some(entry) = entry.filter(|e| e.valid_for(&self.game, ply)) {
                    self.scores[ply] = Some(entry.score);
                    self.bests[ply] = entry.best.and_then(|m| m.parse().ok());
                    self.depths[ply] = entry.depth;
                }
            }
        }
        let last = self.game.history.len();
        if let Some(score) = rule_score(&self.game, last) {
            self.scores[last] = Some(score);
            self.bests[last] = None;
            self.depths[last] = 0;
        }
        self.score_version = self.score_version.wrapping_add(1);
    }
    pub fn analyzed(&mut self, ply: usize, variation: &crate::engine::Variation) {
        if ply >= self.scores.len() {
            return;
        }
        self.scores[ply] = Some(variation.evaluation);
        self.bests[ply] = Some(variation.mv);
        self.depths[ply] = variation.depth;
        self.score_version = self.score_version.wrapping_add(1);
    }
}
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

/// Preserve bucket extrema and every unknown boundary; spikes never disappear into averages.
pub fn chart_samples(scores: &[Option<Score>], columns: usize) -> Vec<(usize, Option<Score>)> {
    let bucket = scores.len().div_ceil(columns.max(1)).max(1);
    let mut indices = std::collections::BTreeSet::new();
    for (group, values) in scores.chunks(bucket).enumerate() {
        let start = group * bucket;
        indices.insert(start);
        indices.insert(start + values.len() - 1);
        for choose_min in [true, false] {
            if let Some((i, _)) = values
                .iter()
                .enumerate()
                .filter_map(|(i, v)| v.map(|s| (i, s.value())))
                .min_by_key(|(_, v)| {
                    if choose_min {
                        i64::from(*v)
                    } else {
                        -i64::from(*v)
                    }
                })
            {
                indices.insert(start + i);
            }
        }
        for (i, score) in values.iter().enumerate() {
            if score.is_none() {
                indices.insert(start + i);
            }
        }
    }
    indices.into_iter().map(|i| (i, scores[i])).collect()
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
