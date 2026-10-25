//! Serial UCI worker. IDs cancel obsolete searches before results can affect a new board.
use crate::rules::{Board, Move, Side};
use std::{
    path::Path,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum Score {
    Cp(i32),
    Mate(i32),
    Terminal(i32),
}

impl Score {
    pub fn is_valid(self) -> bool {
        match self {
            Self::Cp(n) => n.unsigned_abs() <= 100_000,
            Self::Mate(n) => n != 0 && n.unsigned_abs() <= 9999,
            Self::Terminal(n) => n.unsigned_abs() == 1,
        }
    }
    pub fn value(self) -> i32 {
        match self {
            Self::Cp(n) => n.clamp(-100_000, 100_000),
            Self::Mate(n) => {
                if n > 0 {
                    100_000 - n.min(9999)
                } else {
                    -100_000 + n.saturating_abs().min(9999)
                }
            }
            Self::Terminal(n) => n.signum() * 100_000,
        }
    }
    pub fn red_relative(self, turn: Side) -> Self {
        if turn == Side::Red {
            return self;
        }
        match self {
            Self::Cp(n) => Self::Cp(-n),
            Self::Mate(n) => Self::Mate(n.saturating_neg()),
            Self::Terminal(n) => Self::Terminal(-n),
        }
    }
    pub fn label(self) -> String {
        match self {
            Self::Cp(0) => "均势".into(),
            Self::Cp(n) => format!("{} +{} 厘兵", if n > 0 { "红方" } else { "黑方" }, n.abs()),
            Self::Mate(n) => format!(
                "{}强制杀棋（{}）",
                if n > 0 { "红方" } else { "黑方" },
                n.saturating_abs()
            ),
            Self::Terminal(n) => format!("{}胜（规则终局）", if n > 0 { "红方" } else { "黑方" }),
        }
    }
}
/// Mate distances cannot be subtracted as if they were centipawns.
pub fn score_loss(before: Score, after: Score, mover: Side) -> Option<i32> {
    match (before, after) {
        (Score::Cp(a), Score::Cp(b)) => {
            Some((if mover == Side::Red { a - b } else { b - a }).max(0))
        }
        _ => {
            let sign = if mover == Side::Red { 1 } else { -1 };
            let a = before.value() * sign;
            let b = after.value() * sign;
            if b > a && a.signum() != b.signum() {
                Some(0)
            } else if b < a && a.signum() != b.signum() {
                Some(100_000)
            } else {
                None
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveAssessment {
    Centipawns(i32),
    MissedMate,
    AllowsMate,
    EscapedMate,
    FoundMate,
    MateContinuation,
}
impl MoveAssessment {
    pub fn label(self) -> &'static str {
        match self {
            Self::Centipawns(loss) => crate::game::move_quality(loss, false),
            Self::MissedMate => "错失杀棋",
            Self::AllowsMate => "陷入杀棋",
            Self::EscapedMate => "化解杀棋",
            Self::FoundMate => "找到杀棋",
            Self::MateContinuation => "杀棋延续",
        }
    }
    pub fn badge(self) -> &'static str {
        match self.label() {
            "优秀" => "优",
            "良好" => "良",
            "平庸" => "平",
            "较差" => "差",
            "错招" | "错失杀棋" | "陷入杀棋" => "错",
            _ => "杀",
        }
    }
}
pub fn assess_move(before: Score, after: Score, mover: Side) -> MoveAssessment {
    let a = before.red_relative(mover);
    let b = after.red_relative(mover);
    let forced = |s: Score| match s {
        Score::Cp(_) => 0,
        Score::Mate(n) | Score::Terminal(n) => n.signum(),
    };
    match (forced(a), forced(b)) {
        (x, -1) if x != -1 => MoveAssessment::AllowsMate,
        (-1, x) if x != -1 => MoveAssessment::EscapedMate,
        (1, 0) => MoveAssessment::MissedMate,
        (0, 1) => MoveAssessment::FoundMate,
        (0, 0) => MoveAssessment::Centipawns(a.value().saturating_sub(b.value()).max(0)),
        _ => MoveAssessment::MateContinuation,
    }
}
#[derive(Clone)]
pub struct PositionContext {
    pub start: Board,
    pub moves: Vec<Move>,
    pub game_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variation {
    pub mv: Move,
    pub score: i32,
    pub depth: u32,
    pub index: usize,
    pub evaluation: Score,
    pub line: Vec<Move>,
}
#[derive(Clone, Debug)]
pub struct Analysis {
    pub best: Move,
    pub variations: Vec<Variation>,
    pub elapsed_ms: u64,
    pub requested_depth: u32,
    pub budget_ms: u64,
    pub assets: String,
}
/// Stable per-game/per-ply choice: restoring a game never resets its random sequence.
pub fn choose_variation(a: &Analysis, side: Side, tolerance: i32, seed: u64, ply: usize) -> Move {
    if tolerance <= 0 {
        return a.best;
    }
    let sign = if side == Side::Red { 1 } else { -1 };
    let Some(best) = a
        .variations
        .iter()
        .find(|v| v.mv == a.best)
        .map(|v| v.score * sign)
    else {
        return a.best;
    };
    let choices: Vec<_> = a
        .variations
        .iter()
        .filter(|v| best - v.score * sign <= tolerance)
        .collect();
    let total = choices.len() * (choices.len() + 1) / 2;
    if total == 0 {
        return a.best;
    }
    let mut random = seed.wrapping_add((ply as u64).wrapping_mul(0x9e3779b97f4a7c15));
    random = (random ^ (random >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    random = (random ^ (random >> 27)).wrapping_mul(0x94d049bb133111eb);
    random ^= random >> 31;
    let mut slot = (random % total as u64) as usize;
    for (i, v) in choices.iter().enumerate() {
        let weight = choices.len() - i;
        if slot < weight {
            return v.mv;
        }
        slot -= weight;
    }
    a.best
}
#[derive(Clone)]
pub struct Request {
    pub context: Option<PositionContext>,
    pub id: u64,
    pub board: Board,
    pub depth: u32,
    pub ms: u64,
    pub pv: usize,
}
pub struct Reply {
    pub id: u64,
    pub result: Result<Analysis, String>,
    pub failure_kind: Option<crate::diagnostics::FailureKind>,
}
struct Mailbox {
    request: Option<Request>,
    closed: bool,
}
pub struct Engine {
    scripted_replies: mpsc::Sender<Reply>,
    mailbox: Arc<(Mutex<Mailbox>, Condvar)>,
    pub rx: Receiver<Reply>,
    generation: Arc<AtomicU64>,
}
impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
impl Engine {
    pub fn new() -> Self {
        let mailbox = Arc::new((
            Mutex::new(Mailbox {
                request: None,
                closed: false,
            }),
            Condvar::new(),
        ));
        let (replies, rx) = mpsc::channel();
        let scripted_replies = replies.clone();
        let generation = Arc::new(AtomicU64::new(0));
        let token = generation.clone();
        let worker_mailbox = mailbox.clone();
        std::thread::spawn(move || {
            let mut session: Option<Session> = None;
            loop {
                let req = {
                    let (lock, ready) = &*worker_mailbox;
                    let Ok(mut state) = lock.lock() else {
                        return;
                    };
                    while state.request.is_none() && !state.closed {
                        state = match ready.wait(state) {
                            Ok(s) => s,
                            Err(_) => return,
                        };
                    }
                    if state.closed {
                        return;
                    }
                    state.request.take().unwrap()
                };
                if token.load(Ordering::Acquire) != req.id {
                    continue;
                }
                let mut result = Err("引擎不可用".into());
                let mut failure_kind = crate::diagnostics::FailureKind::Other;
                // One deadline includes cold start, handshake, all writes and both attempts.
                let deadline =
                    Instant::now() + Duration::from_millis(req.ms.clamp(1, 60_000) + 15_000);
                for _ in 0..2 {
                    result = (|| {
                        if session.is_none() {
                            session = Some(Session::open(&token, req.id, deadline).map_err(
                                |(kind, message)| {
                                    failure_kind = kind;
                                    message
                                },
                            )?);
                        }
                        let current = session.as_mut().unwrap();
                        current.deadline = Some(deadline);
                        let result = current.search(&req, &token);
                        failure_kind = current.failure_kind;
                        result
                    })();
                    if result.is_ok() {
                        break;
                    }
                    if crate::platform::suspended() {
                        session = None;
                        break;
                    }
                    session = None;
                    if token.load(Ordering::Acquire) != req.id
                        || Instant::now() >= deadline
                        || matches!(
                            failure_kind,
                            crate::diagnostics::FailureKind::RestartRequired
                        )
                    {
                        break;
                    }
                }
                if let Err(e) = &result {
                    crate::diagnostics::record(
                        "search",
                        &format!("request={} {:?}: {e}", req.id, failure_kind),
                    );
                }
                if token.load(Ordering::Acquire) == req.id
                    && replies
                        .send(Reply {
                            id: req.id,
                            failure_kind: result.as_ref().err().map(|_| failure_kind),
                            result,
                        })
                        .is_err()
                {
                    return;
                }
            }
        });
        Self {
            scripted_replies,
            mailbox,
            rx,
            generation,
        }
    }
    pub(crate) fn smoke_reply(&self, reply: Reply) {
        let _ = self.scripted_replies.send(reply);
    }
    pub fn cancel(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }
    pub fn request(&self, board: Board, depth: u32, ms: u64, pv: usize) -> Result<u64, String> {
        self.submit(board, None, depth, ms, pv)
    }
    pub fn request_game(
        &self,
        game: &crate::game::Game,
        ply: usize,
        depth: u32,
        ms: u64,
        pv: usize,
    ) -> Result<u64, String> {
        let board = game.at_ply(ply).ok_or("分析步数无效")?.clone();
        let context = PositionContext {
            start: game.start.clone(),
            moves: game.history.iter().take(ply).map(|e| e.mv).collect(),
            game_id: game.id.clone(),
        };
        self.submit(board, Some(context), depth, ms, pv)
    }
    fn submit(
        &self,
        board: Board,
        context: Option<PositionContext>,
        depth: u32,
        ms: u64,
        pv: usize,
    ) -> Result<u64, String> {
        let (lock, ready) = &*self.mailbox;
        let mut state = lock.lock().map_err(|_| "引擎队列已损坏")?;
        if state.closed {
            return Err("引擎已关闭".into());
        }
        // An atomic latest-request mailbox never cancels a request just because a queue is full.
        let id = self.cancel();
        state.request = Some(Request {
            id,
            board,
            context,
            depth,
            ms: ms.clamp(1, 60_000),
            pv: pv.clamp(1, 5),
        });
        ready.notify_one();
        Ok(id)
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        self.cancel();
        let (lock, ready) = &*self.mailbox;
        if let Ok(mut state) = lock.lock() {
            state.closed = true;
            state.request = None;
            ready.notify_one();
        }
    }
}

mod protocol;
pub use protocol::{Session, Transport, parse_info};
#[cfg(not(any(target_os = "ios", target_os = "android")))]
mod process;
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use process::Process;
#[cfg(any(target_os = "ios", target_os = "android"))]
mod mobile;
