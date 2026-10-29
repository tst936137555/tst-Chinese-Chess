//! Pure review scheduling: the viewer cursor never follows background progress.
use crate::{engine::Score, game::Game};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub struct ChartCache {
    pub version: u64,
    pub columns: usize,
    pub points: Vec<(usize, Option<Score>)>,
    pub mistakes: Vec<usize>,
}
pub struct ReviewSession {
    pub game: Game,
    pub ply: usize,
    pub scores: Vec<Option<Score>>,
    pub depths: Vec<u32>,
    pub details: Vec<Option<AnalysisDetails>>,
    pub cache_key: String,
    pub cache_loading: bool,
    pub bests: Vec<Option<crate::rules::Move>>,
    pub analyzing: bool,
    pub user_red: bool,
    pub score_version: u64,
    pub chart_cache: ChartCache,
    pub dirty: std::collections::BTreeSet<usize>,
    pub attempted: Vec<bool>,
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
            details: vec![None; n],
            cache_loading: true,
            bests: vec![None; n],
            analyzing: true,
            user_red,
            score_version: 0,
            chart_cache: ChartCache {
                version: u64::MAX,
                columns: 0,
                points: Vec::new(),
                mistakes: Vec::new(),
            },
            dirty: Default::default(),
            attempted: vec![false; n],
        }
    }
    pub fn cache_loaded(&mut self, cache: Option<CachedReview>) {
        self.cache_loading = false;
        if let Some(cache) = cache.filter(|c| c.entries.len() == self.scores.len()) {
            for (ply, entry) in cache.entries.into_iter().enumerate() {
                if let Some(entry) = entry.filter(|e| {
                    e.valid_for(&self.game, ply)
                        && (self.scores[ply].is_none()
                            || e.depth > self.depths[ply]
                            || (e.depth == self.depths[ply]
                                && self.details[ply].is_none()
                                && !self.attempted[ply]
                                && e.details.is_some()))
                }) {
                    self.scores[ply] = Some(entry.score);
                    self.bests[ply] = entry.best.and_then(|m| m.parse().ok());
                    self.depths[ply] = entry.depth;
                    self.details[ply] = entry.details;
                }
            }
        }
        let last = self.game.history.len();
        if let Some(score) = rule_score(&self.game, last) {
            self.scores[last] = Some(score);
            self.bests[last] = None;
            self.depths[last] = 0;
            self.details[last] = None;
            self.dirty.insert(last);
        }
        self.score_version = self.score_version.wrapping_add(1);
    }
    pub fn analyzed(&mut self, ply: usize, variation: &crate::engine::Variation) {
        if ply >= self.scores.len() {
            return;
        }
        self.attempted[ply] = true;
        if self.scores[ply].is_some() && self.depths[ply] > variation.depth {
            return;
        }
        self.details[ply] = None;
        self.scores[ply] = Some(variation.evaluation);
        self.bests[ply] = Some(variation.mv);
        self.depths[ply] = variation.depth;
        self.dirty.insert(ply);
        self.score_version = self.score_version.wrapping_add(1);
    }
    pub fn complete(&self, ply: usize) -> bool {
        self.attempted[ply]
            || rule_score(&self.game, ply).is_some()
            || self.details[ply]
                .as_ref()
                .is_some_and(AnalysisDetails::current)
    }
    pub fn analysis_complete(&self) -> bool {
        !self.cache_loading
            && (0..self.scores.len()).all(|ply| self.complete(ply) && self.scores[ply].is_some())
    }
    /// One assessment for the board badge, detail text, chart and mistake navigation.
    pub fn assessment(&self, ply: usize) -> Option<crate::engine::MoveAssessment> {
        use crate::engine::{MoveAssessment, assess_move};
        let index = ply.checked_sub(1)?;
        let entry = self.game.history.get(index)?;
        let assessment = self.scores[index]
            .zip(self.scores[ply])
            .map(|(before, after)| assess_move(before, after, entry.mover));
        // A newly discovered forced loss takes priority over an earlier best-move result.
        if assessment == Some(MoveAssessment::AllowsMate) {
            assessment
        } else if self.bests[index] == Some(entry.mv) {
            Some(MoveAssessment::Centipawns(0))
        } else {
            assessment
        }
    }
    pub fn mistake_plies(&self) -> Vec<usize> {
        (1..=self.game.history.len())
            .filter(|&ply| self.assessment(ply).is_some_and(|a| a.is_mistake()))
            .collect()
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
pub const ANALYSIS_LIMITS: (u32, u64) = (crate::game::LEVELS[4].depth, crate::game::LEVELS[4].ms);
#[derive(Clone, Serialize, Deserialize)]
pub struct AnalysisDetails {
    pub line: Vec<String>,
    pub elapsed_ms: u64,
    pub budget_ms: u64,
    pub requested_depth: u32,
}
impl AnalysisDetails {
    pub fn current(&self) -> bool {
        self.requested_depth == ANALYSIS_LIMITS.0 && self.budget_ms == ANALYSIS_LIMITS.1
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct CachedPosition {
    pub score: Score,
    pub best: Option<String>,
    pub depth: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<AnalysisDetails>,
}
impl CachedPosition {
    pub fn supersedes(&self, old: &Self) -> bool {
        self.depth > old.depth
            || (self.depth == old.depth && (self.details.is_some() || old.details.is_none()))
    }
    pub fn valid_for(&self, game: &Game, ply: usize) -> bool {
        if !self.score.is_valid() || self.depth > 64 {
            return false;
        }
        if let Some(details) = &self.details {
            if details.line.len() > 32
                || details.elapsed_ms > 120_000
                || details.budget_ms > 60_000
                || details.requested_depth > 64
                || details.line.first() != self.best.as_ref()
            {
                return false;
            }
            let Some(mut board) = game.at_ply(ply).cloned() else {
                return false;
            };
            for word in &details.line {
                if word.parse().ok().is_none_or(|m| board.play(m).is_err()) {
                    return false;
                }
            }
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
    hash.update(b"review-v3\0");
    hash.update(crate::game::RULESET);
    hash.update(ANALYSIS_LIMITS.0.to_le_bytes());
    hash.update(ANALYSIS_LIMITS.1.to_le_bytes());
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

/// Keep marked moves on the actual polyline even when long games are downsampled.
pub fn marked_chart_samples(
    scores: &[Option<Score>],
    columns: usize,
    markers: &[usize],
) -> Vec<(usize, Option<Score>)> {
    let mut points: std::collections::BTreeMap<_, _> =
        chart_samples(scores, columns).into_iter().collect();
    for &ply in markers {
        if let Some(&score) = scores.get(ply) {
            points.insert(ply, score);
        }
    }
    points.into_iter().collect()
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
