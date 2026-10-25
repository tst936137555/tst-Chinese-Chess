use crate::{
    notation::chinese,
    rules::{Board, Move, START_FEN, Side},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};

pub const MAX_PLIES: usize = 4096;
pub const MAX_LINEAGE: usize = MAX_PLIES * 4;
pub const RULESET: &str = "simplified-v1";
pub const MAX_REVISION: u64 = u64::MAX - 1_000_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EndReason {
    Checkmate,
    Stalemate,
    Repetition,
    PerpetualCheck {
        loser: Side,
    },
    NoCapture,
    Resignation {
        side: Side,
    },
    Evaluation {
        score: i32,
        depth: u32,
        ms: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        evidence: Option<Box<EvaluationEvidence>>,
    },
    Legacy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationEvidence {
    pub score: crate::engine::Score,
    pub requested_depth: u32,
    pub reached_depth: u32,
    pub budget_ms: u64,
    pub elapsed_ms: u64,
    pub assets: String,
}

static ID_CLOCK: AtomicU64 = AtomicU64::new(0);
pub(crate) fn advance_identity_clock(floor: u64) {
    ID_CLOCK.fetch_max(floor, Ordering::Relaxed);
}
pub(crate) fn identity_time(id: &str) -> Option<u64> {
    let mut parts = id.split('-');
    let time = u64::from_str_radix(parts.next()?, 16).ok()?;
    parts.next()?;
    parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some(time)
}
pub fn new_id() -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let wall = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .min(u128::from(u64::MAX - 1_000_000)) as u64;
    let previous = ID_CLOCK
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |last| {
            Some(last.max(wall).saturating_add(1))
        })
        .unwrap();
    let nanos = previous.max(wall).saturating_add(1);
    format!(
        "{nanos:x}-{:x}-{:x}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    #[default]
    Playing,
    RedWin,
    BlackWin,
    Draw,
}
pub fn outcome_by_score(score: i32) -> Outcome {
    if score > 500 {
        Outcome::RedWin
    } else if score < -500 {
        Outcome::BlackWin
    } else {
        Outcome::Draw
    }
}
pub fn move_quality(loss: i32, is_best: bool) -> &'static str {
    if is_best || loss < 30 {
        "优秀"
    } else if loss < 100 {
        "良好"
    } else if loss < 250 {
        "平庸"
    } else if loss < 600 {
        "较差"
    } else {
        "错招"
    }
}
impl Outcome {
    pub fn winner(side: Side) -> Self {
        if side == Side::Red {
            Self::RedWin
        } else {
            Self::BlackWin
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Playing => "对局进行中",
            Self::RedWin => "红方胜",
            Self::BlackWin => "黑方胜",
            Self::Draw => "和棋",
        }
    }
}
#[derive(Clone, Debug)]
pub struct Entry {
    pub mv: Move,
    pub notation: String,
    pub board: Board,
    pub mover: Side,
    pub check: bool,
    pub capture: bool,
}
#[derive(Clone, Debug)]
pub struct Game {
    pub source: RecordSource,
    pub id: String,
    pub created_at: u64,
    pub revision: u64,
    pub version_token: String,
    pub lineage: Vec<String>,
    pub random_seed: u64,
    pub finished_at: Option<u64>,
    pub end_reason: Option<EndReason>,
    position_keys: Vec<String>,
    tracking: bool,
    pub start: Board,
    pub board: Board,
    pub history: Vec<Entry>,
    pub outcome: Outcome,
    pub reason: String,
}
impl Default for Game {
    fn default() -> Self {
        Self::new(Board::default())
    }
}
impl Game {
    pub fn new(board: Board) -> Self {
        Self::build(board, true)
    }
    fn build(board: Board, tracking: bool) -> Self {
        let mut g = Self {
            source: RecordSource::Native,
            id: if tracking { new_id() } else { String::new() },
            created_at: now_ms(),
            revision: 0,
            version_token: if tracking { new_id() } else { String::new() },
            lineage: Vec::new(),
            random_seed: now_ms() ^ SEED_COUNTER.fetch_add(1, Ordering::Relaxed),
            finished_at: None,
            end_reason: None,
            position_keys: vec![board.position_key()],
            tracking,
            start: board.clone(),
            board,
            history: Vec::new(),
            outcome: Outcome::Playing,
            reason: String::new(),
        };
        g.adjudicate();
        g
    }
    fn changed(&mut self) {
        if !self.tracking {
            return;
        }
        self.lineage
            .push(std::mem::replace(&mut self.version_token, new_id()));
        if self.lineage.len() > MAX_LINEAGE {
            self.lineage.remove(0);
        }
        // Imported counters are bounded; public callers cannot cause a panic either.
        self.revision = self.revision.saturating_add(1).min(MAX_REVISION);
    }
    pub fn play(&mut self, m: Move) -> Result<(), String> {
        if self.outcome != Outcome::Playing {
            return Err("对局已结束".into());
        }
        if self.history.len() >= MAX_PLIES {
            return Err("棋谱超过最大步数，请保存后另开对局".into());
        }
        let notation = chinese(&self.board, m)?;
        let mover = self.board.turn;
        let capture = self.board.play(m)?.is_some();
        let check = self.board.in_check(self.board.turn);
        self.history.push(Entry {
            mv: m,
            notation,
            board: self.board.clone(),
            mover,
            check,
            capture,
        });
        self.position_keys.push(self.board.position_key());
        self.changed();
        self.adjudicate();
        Ok(())
    }
    fn adjudicate(&mut self) {
        if !self.board.has_legal_move() {
            self.finished_at = Some(now_ms());
            self.outcome = Outcome::winner(self.board.turn.other());
            self.end_reason = Some(if self.board.in_check(self.board.turn) {
                EndReason::Checkmate
            } else {
                EndReason::Stalemate
            });
            self.reason = if self.board.in_check(self.board.turn) {
                "将死"
            } else {
                "困毙"
            }
            .into();
            return;
        }
        if let Some((outcome, reason)) = self.repetition() {
            self.finished_at = Some(now_ms());
            self.outcome = outcome;
            self.end_reason = Some(match outcome {
                Outcome::BlackWin => EndReason::PerpetualCheck { loser: Side::Red },
                Outcome::RedWin => EndReason::PerpetualCheck { loser: Side::Black },
                _ => EndReason::Repetition,
            });
            self.reason = reason.into();
            return;
        }
        if self.board.halfmove >= 120 {
            self.finished_at = Some(now_ms());
            self.outcome = Outcome::Draw;
            self.reason = "60 回合无吃子".into();
            self.end_reason = Some(EndReason::NoCapture);
        }
    }
    /// Inspect both repeated intervals, using the recorded mover rather than ply parity.
    fn repetition(&self) -> Option<(Outcome, &'static str)> {
        let key = self.position_keys.last().unwrap();
        let occurrences: Vec<_> = self
            .position_keys
            .iter()
            .enumerate()
            .filter(|(_, b)| *b == key)
            .map(|(i, _)| i)
            .collect();
        if occurrences.len() < 3 {
            return None;
        }
        let start = occurrences[occurrences.len() - 3];
        let entries = &self.history[start..];
        let all_checks = |side| {
            let turns: Vec<_> = entries.iter().filter(|e| e.mover == side).collect();
            !turns.is_empty() && turns.iter().all(|e| e.check)
        };
        match (all_checks(Side::Red), all_checks(Side::Black)) {
            (true, false) => Some((Outcome::BlackWin, "红方长将判负")),
            (false, true) => Some((Outcome::RedWin, "黑方长将判负")),
            _ => Some((Outcome::Draw, "三次重复局面")),
        }
    }
    pub fn repetition_count(&self) -> usize {
        self.position_keys.last().map_or(0, |last| {
            self.position_keys.iter().filter(|k| *k == last).count()
        })
    }
    pub fn at_ply(&self, ply: usize) -> Option<&Board> {
        if ply == 0 {
            Some(&self.start)
        } else {
            self.history.get(ply - 1).map(|e| &e.board)
        }
    }
    pub fn undo_to_player(&mut self, player: Side) {
        if self.history.is_empty() {
            return;
        }
        self.history.pop();
        while !self.history.is_empty()
            && self
                .history
                .last()
                .map_or(self.start.turn, |e| e.board.turn)
                != player
        {
            self.history.pop();
        }
        self.board = self
            .history
            .last()
            .map_or_else(|| self.start.clone(), |e| e.board.clone());
        self.position_keys.truncate(self.history.len() + 1);
        self.outcome = Outcome::Playing;
        self.reason.clear();
        self.end_reason = None;
        self.finished_at = None;
        self.changed();
        self.adjudicate();
    }
    pub fn resign(&mut self, side: Side) {
        if self.outcome == Outcome::Playing {
            self.outcome = Outcome::winner(side.other());
            self.finished_at = Some(now_ms());
            self.reason = format!("{}认输", side.label());
            self.end_reason = Some(EndReason::Resignation { side });
            self.changed();
        }
    }
    pub fn settle(&mut self, score: i32, depth: u32, ms: u64) {
        if self.outcome == Outcome::Playing {
            self.outcome = outcome_by_score(score);
            self.finished_at = Some(now_ms());
            self.reason = "按当前引擎评分结算".into();
            self.end_reason = Some(EndReason::Evaluation {
                score,
                depth,
                ms,
                evidence: None,
            });
            self.changed();
        }
    }
    pub fn settle_evaluated(&mut self, evidence: EvaluationEvidence) {
        if self.outcome != Outcome::Playing {
            return;
        }
        self.settle(
            evidence.score.value(),
            evidence.reached_depth,
            evidence.budget_ms,
        );
        if let Some(EndReason::Evaluation {
            evidence: saved, ..
        }) = &mut self.end_reason
        {
            *saved = Some(Box::new(evidence));
        }
    }
    pub fn fork_identity(&mut self) {
        self.id = new_id();
        self.version_token = new_id();
        self.lineage.clear();
        self.revision = 0;
        self.created_at = now_ms();
        if self.finished_at.is_some() {
            self.finished_at = Some(now_ms());
        }
    }
    /// A branch retains the original move history for repetition and engine search.
    pub fn can_branch_at(&self, ply: usize) -> bool {
        ply <= self.history.len()
            && (ply < self.history.len()
                || self.outcome == Outcome::Playing
                || matches!(
                    self.end_reason,
                    Some(
                        EndReason::Resignation { .. }
                            | EndReason::Evaluation { .. }
                            | EndReason::Legacy
                    )
                ))
    }
    pub fn branch_at(&self, ply: usize) -> Result<Self, String> {
        if ply > self.history.len() {
            return Err("续下步数无效".into());
        }
        let mut g = Game::new(self.start.clone());
        for entry in self.history.iter().take(ply) {
            g.play(entry.mv)?;
        }
        if g.outcome != Outcome::Playing {
            return Err("终局不能继续走棋".into());
        }
        Ok(g)
    }
    pub fn snapshot(&self, user_red: bool, level: usize, review: bool) -> Record {
        Record {
            ruleset: RULESET.into(),
            source: self.source,
            id: self.id.clone(),
            revision: self.revision,
            version_token: self.version_token.clone(),
            lineage: self.lineage.clone(),
            random_seed: self.random_seed,
            created_at: self.created_at,
            finished_at: self.finished_at,
            updated_at: now_ms(),
            end_reason: self.end_reason.clone(),
            time: self.created_at,
            user_red,
            level_name: LEVELS[level.min(4)].name.into(),
            result: self.outcome,
            history: self
                .history
                .iter()
                .map(|e| SavedMove {
                    uci: e.mv.to_string(),
                })
                .collect(),
            favorite: false,
            favorite_revision: 0,
            start_fen: Some(self.start.fen()),
            mode: if review { "review" } else { "normal" }.into(),
            title: String::new(),
            title_revision: 0,
        }
    }
}
static SEED_COUNTER: AtomicU64 = AtomicU64::new(0x9e3779b97f4a7c15);
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedMove {
    pub uci: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    #[serde(default = "default_ruleset")]
    pub ruleset: String,
    #[serde(default)]
    pub source: RecordSource,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub title_revision: u64,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub version_token: String,
    #[serde(default)]
    pub lineage: Vec<String>,
    #[serde(default)]
    pub random_seed: u64,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub finished_at: Option<u64>,
    #[serde(default)]
    pub end_reason: Option<EndReason>,
    pub time: u64,
    #[serde(default = "yes")]
    pub user_red: bool,
    #[serde(default)]
    pub level_name: String,
    #[serde(default)]
    pub result: Outcome,
    pub history: Vec<SavedMove>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub favorite_revision: u64,
    #[serde(default)]
    pub start_fen: Option<String>,
    #[serde(default = "normal")]
    pub mode: String,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecordSource {
    #[default]
    Native,
    Legacy,
}
fn yes() -> bool {
    true
}
fn default_ruleset() -> String {
    RULESET.into()
}
fn normal() -> String {
    "normal".into()
}
impl Record {
    pub(crate) fn semantic_key(&self) -> [u8; 32] {
        Sha256::digest(
            serde_json::to_vec(&(
                &self.ruleset,
                &self.start_fen,
                &self.history,
                self.result,
                &self.end_reason,
                self.id.is_empty(),
                self.source,
            ))
            .expect("replay input"),
        )
        .into()
    }
    pub fn content_key(&self) -> String {
        let payload = serde_json::to_vec(&(
            self.time,
            self.start_fen.as_deref().unwrap_or(START_FEN),
            self.history.iter().map(|m| &m.uci).collect::<Vec<_>>(),
            self.result,
            &self.end_reason,
        ))
        .expect("record payload");
        format!("{:x}", Sha256::digest(payload))
    }
    /// A later write only acknowledges this exact state or a proven descendant.
    pub fn acknowledges(&self, pending: &Self) -> bool {
        self.id == pending.id
            && ((self.version_token == pending.version_token
                && self.content_key() == pending.content_key())
                || self.lineage.contains(&pending.version_token))
    }
    /// Legacy identity is deterministic, independent of viewing side.
    pub fn normalize(&mut self) {
        if self.id.is_empty() || (self.id.starts_with("legacy-") && self.version_token.is_empty()) {
            self.source = RecordSource::Legacy;
        }
        if self.id.is_empty() {
            self.id = format!("legacy-{}", self.content_key());
        }
        if self.version_token.is_empty() {
            self.version_token = format!("migrated-{}-{}", self.revision, self.content_key());
        }
        if self.random_seed == 0 {
            let digest = Sha256::digest(self.id.as_bytes());
            self.random_seed = u64::from_le_bytes(digest[..8].try_into().unwrap());
        }
        if self.created_at == 0 {
            self.created_at = self.time;
        }
        if self.updated_at == 0 {
            self.updated_at = self.time;
        }
        if self.result != Outcome::Playing && self.finished_at.is_none() {
            self.finished_at = Some(self.updated_at);
        }
    }
    /// Shared by readers, importers and transaction commits.
    pub fn validate_metadata(&self) -> Result<(), String> {
        if self.ruleset != RULESET {
            return Err("棋谱使用不支持的裁决版本，请升级程序，原文件已保留".into());
        }
        if self.revision > MAX_REVISION {
            return Err("棋谱修订号超出有效范围，原文件已保留".into());
        }
        if self.history.len() > MAX_PLIES {
            return Err("棋谱超过 4096 步".into());
        }
        if self.id.len() > 128
            || self.level_name.len() > 128
            || self.title.chars().count() > 40
            || self.version_token.len() > 160
            || self.lineage.len() > MAX_LINEAGE
            || self.lineage.iter().any(|t| t.len() > 160)
            || self.mode.len() > 32
            || self.start_fen.as_ref().is_some_and(|f| f.len() > 256)
            || self.history.iter().any(|m| m.uci.parse::<Move>().is_err())
            || self.title.chars().any(char::is_control)
        {
            return Err("棋谱元数据无效，原文件已保留".into());
        }
        Ok(())
    }
    pub fn replay(&self) -> Result<Game, String> {
        self.validate_metadata()?;
        let mut g = Game::build(
            self.start_fen.as_deref().unwrap_or(START_FEN).parse()?,
            false,
        );
        for (i, m) in self.history.iter().enumerate() {
            g.play(m.uci.parse()?)
                .map_err(|e| format!("第 {} 步：{e}", i + 1))?;
        }
        if g.outcome != Outcome::Playing {
            if self.result != g.outcome
                && !self.id.is_empty()
                && self.source != RecordSource::Legacy
            {
                return Err("棋谱结果与规则终局不一致".into());
            }
            if self
                .end_reason
                .as_ref()
                .is_some_and(|r| Some(r) != g.end_reason.as_ref())
            {
                return Err("棋谱终局原因与重放不一致".into());
            }
        } else if self.result != Outcome::Playing {
            match &self.end_reason {
                Some(EndReason::Resignation { side })
                    if self.result == Outcome::winner(side.other()) =>
                {
                    g.resign(*side)
                }
                Some(EndReason::Evaluation {
                    score,
                    depth,
                    ms,
                    evidence,
                }) if self.result == outcome_by_score(*score)
                    && *depth > 0
                    && *depth <= 64
                    && *ms <= 60_000 =>
                {
                    if let Some(e) = evidence {
                        if !e.score.is_valid()
                            || e.score.value() != *score
                            || e.reached_depth != *depth
                            || e.budget_ms != *ms
                            || e.requested_depth > 64
                            || e.reached_depth > e.requested_depth
                            || e.assets.len() > 256
                            || e.elapsed_ms > 120_000
                        {
                            return Err("结算分析依据无效".into());
                        }
                        g.settle_evaluated((**e).clone());
                    } else {
                        g.settle(*score, *depth, *ms);
                    }
                }
                None | Some(EndReason::Legacy)
                    if !self.history.is_empty()
                        && (self.id.is_empty() || self.source == RecordSource::Legacy) =>
                {
                    g.outcome = self.result;
                    g.reason = "旧版人工终局（原因未记录）".into();
                    g.end_reason = Some(EndReason::Legacy);
                }
                _ => return Err("棋谱缺少有效的人工终局依据".into()),
            }
        } else if self.end_reason.is_some() {
            return Err("进行中的棋谱不应包含终局原因".into());
        }
        let mut normalized = self.clone();
        normalized.normalize();
        g.id = normalized.id;
        g.source = normalized.source;
        g.created_at = normalized.created_at;
        g.revision = self.revision;
        g.version_token = normalized.version_token;
        g.lineage = normalized.lineage;
        g.random_seed = normalized.random_seed;
        g.tracking = true;
        g.finished_at = if g.outcome == Outcome::Playing {
            None
        } else {
            normalized.finished_at.or(Some(normalized.updated_at))
        };
        Ok(g)
    }
}
#[derive(Clone, Copy)]
pub struct Level {
    pub name: &'static str,
    pub depth: u32,
    pub ms: u64,
    pub pv: usize,
    pub tolerance: i32,
}
pub const LEVELS: [Level; 5] = [
    Level {
        name: "入门",
        depth: 3,
        ms: 1000,
        pv: 5,
        tolerance: 300,
    },
    Level {
        name: "简单",
        depth: 5,
        ms: 1500,
        pv: 4,
        tolerance: 180,
    },
    Level {
        name: "中等",
        depth: 7,
        ms: 2000,
        pv: 3,
        tolerance: 100,
    },
    Level {
        name: "困难",
        depth: 9,
        ms: 2500,
        pv: 2,
        tolerance: 40,
    },
    Level {
        name: "大师",
        depth: 12,
        ms: 3000,
        pv: 1,
        tolerance: 0,
    },
];
