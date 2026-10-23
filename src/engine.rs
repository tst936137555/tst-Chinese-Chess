//! Serial UCI worker. IDs cancel obsolete searches before results can affect a new board.
use crate::rules::{Board, Move, Side};
#[cfg(not(any(target_os = "ios", target_os = "android")))]
use std::io::{BufRead, BufReader, Read, Write};
use std::{
    collections::BTreeMap,
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
                for _ in 0..2 {
                    result = (|| {
                        if session.is_none() {
                            session = Some(Session::open(&token, req.id).map_err(
                                |(kind, message)| {
                                    failure_kind = kind;
                                    message
                                },
                            )?);
                        }
                        let current = session.as_mut().unwrap();
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
                    if token.load(Ordering::Acquire) != req.id {
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
                    && replies.send(Reply { id: req.id, result }).is_err()
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

pub trait Transport: Send {
    fn send(&mut self, line: &str) -> Result<(), String>;
    fn receive(&mut self, timeout: Duration) -> Result<Option<String>, String>;
}
pub struct Session {
    io: Box<dyn Transport>,
    game_id: Option<String>,
    assets: String,
    failure_kind: crate::diagnostics::FailureKind,
}
impl Session {
    pub fn with_transport(io: Box<dyn Transport>) -> Self {
        Self {
            io,
            game_id: None,
            assets: "test-transport".into(),
            failure_kind: crate::diagnostics::FailureKind::Protocol,
        }
    }
    fn open(token: &AtomicU64, id: u64) -> Result<Self, (crate::diagnostics::FailureKind, String)> {
        let asset_error = |e| (crate::diagnostics::FailureKind::Assets, e);
        let nnue = crate::platform::nnue_path().map_err(asset_error)?;
        #[cfg(any(target_os = "ios", target_os = "android"))]
        let io = mobile::Ffi::open().map_err(asset_error)?;
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let io = Process::open(&crate::platform::engine_path().map_err(asset_error)?)
            .map_err(asset_error)?;
        let mut session = Self::with_transport(Box::new(io));
        session.assets = crate::persistence::asset_fingerprint().map_err(asset_error)?;
        session
            .handshake(&nnue, token, id)
            .map_err(|e| (session.failure_kind, e))?;
        Ok(session)
    }
    pub fn handshake(&mut self, nnue: &Path, token: &AtomicU64, id: u64) -> Result<(), String> {
        let path = nnue.to_str().ok_or("NNUE 路径编码无效")?;
        if path.contains(['\r', '\n']) {
            return Err("NNUE 路径包含换行".into());
        }
        self.io.send("uci")?;
        self.wait_for("uciok", Duration::from_secs(10), token, id)?;
        self.io.send("setoption name Threads value 1")?;
        self.io.send("setoption name Hash value 32")?;
        self.io
            .send(&format!("setoption name EvalFile value {path}"))?;
        self.io.send("isready")?;
        self.wait_for("readyok", Duration::from_secs(10), token, id)
    }
    fn wait_for(
        &mut self,
        expected: &str,
        timeout: Duration,
        token: &AtomicU64,
        id: u64,
    ) -> Result<(), String> {
        let end = Instant::now() + timeout;
        loop {
            if let Some(line) = self.next(end, token, id)?
                && line.trim() == expected
            {
                return Ok(());
            }
        }
    }
    fn next(&mut self, end: Instant, token: &AtomicU64, id: u64) -> Result<Option<String>, String> {
        if crate::platform::suspended() {
            self.failure_kind = crate::diagnostics::FailureKind::Cancelled;
            return Err("应用已暂停，搜索已取消".into());
        }
        if token.load(Ordering::Acquire) != id {
            self.failure_kind = crate::diagnostics::FailureKind::Cancelled;
            return Err("搜索已取消".into());
        }
        let left = end.checked_duration_since(Instant::now()).ok_or_else(|| {
            self.failure_kind = crate::diagnostics::FailureKind::Timeout;
            "引擎响应超时"
        })?;
        self.io.receive(left.min(Duration::from_millis(40)))
    }
    pub fn search(&mut self, req: &Request, token: &AtomicU64) -> Result<Analysis, String> {
        self.failure_kind = crate::diagnostics::FailureKind::Protocol;
        let started = Instant::now();
        if let Some(ctx) = &req.context
            && self.game_id.as_ref() != Some(&ctx.game_id)
        {
            self.io.send("ucinewgame")?;
            self.game_id = Some(ctx.game_id.clone());
        }
        self.io.send(&format!(
            "setoption name MultiPV value {}",
            req.pv.clamp(1, 5)
        ))?;
        self.io.send("isready")?;
        self.wait_for("readyok", Duration::from_secs(5), token, req.id)?;
        let position = if let Some(ctx) = &req.context {
            if ctx.moves.len() > crate::game::MAX_PLIES {
                return Err("引擎历史过长".into());
            }
            let mut verified = ctx.start.clone();
            for m in &ctx.moves {
                verified.play(*m)?;
            }
            if verified != req.board {
                return Err("引擎历史与当前局面不一致".into());
            }
            format!(
                "position fen {} moves {}",
                ctx.start.fen(),
                ctx.moves
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        } else {
            format!("position fen {}", req.board.fen())
        };
        self.io.send(&position)?;
        self.io.send(&format!(
            "go depth {} movetime {}",
            req.depth.clamp(1, 64),
            req.ms.clamp(1, 60_000)
        ))?;
        let end = Instant::now() + Duration::from_millis(req.ms.clamp(1, 60_000) + 5000);
        let mut iterations: BTreeMap<u32, BTreeMap<usize, Variation>> = BTreeMap::new();
        let expected = req.pv.clamp(1, 5).min(req.board.legal_moves().len());
        loop {
            let Some(line) = self.next(end, token, req.id)? else {
                continue;
            };
            if let Some(v) = parse_info(&line, &req.board) {
                iterations.entry(v.depth).or_default().insert(v.index, v);
            }
            if line.starts_with("bestmove ") {
                let best: Move = line
                    .split_whitespace()
                    .nth(1)
                    .ok_or("引擎走法为空")?
                    .parse()?;
                if !req.board.is_legal(best) {
                    return Err(format!("引擎返回非法走法：{best}"));
                }
                // Select one complete iteration. If none is complete, expose only the final
                // best move with its own score, never stale lower-ranked alternatives.
                let complete = iterations.iter().rev().find(|(_, row)| {
                    row.get(&1).is_some_and(|v| v.mv == best)
                        && (1..=expected).all(|i| row.contains_key(&i))
                        && row
                            .values()
                            .filter(|v| v.index <= expected)
                            .map(|v| v.mv)
                            .collect::<Vec<_>>()
                            .iter()
                            .enumerate()
                            .all(|(i, m)| {
                                !row.values()
                                    .filter(|v| v.index <= expected)
                                    .take(i)
                                    .any(|v| &v.mv == m)
                            })
                });
                let variations = if let Some((_, row)) = complete {
                    row.values()
                        .filter(|v| v.index <= expected)
                        .cloned()
                        .collect()
                } else {
                    let v = iterations
                        .values()
                        .rev()
                        .flat_map(|row| row.values())
                        .find(|v| v.mv == best)
                        .ok_or("引擎没有返回最佳走法的有效评分")?;
                    vec![v.clone()]
                };
                crate::diagnostics::record(
                    "search-complete",
                    &format!(
                        "request={} game={} plies={} requested_depth={} reached_depth={} budget_ms={} elapsed_ms={} assets={}",
                        req.id,
                        req.context.as_ref().map_or("study", |c| c.game_id.as_str()),
                        req.context.as_ref().map_or(0, |c| c.moves.len()),
                        req.depth,
                        variations.first().map_or(0, |v| v.depth),
                        req.ms,
                        started.elapsed().as_millis(),
                        self.assets
                    ),
                );
                return Ok(Analysis {
                    best,
                    variations,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    requested_depth: req.depth.clamp(1, 64),
                    budget_ms: req.ms.clamp(1, 60000),
                    assets: self.assets.clone(),
                });
            }
        }
    }
}
pub fn parse_info(line: &str, board: &Board) -> Option<Variation> {
    let words: Vec<_> = line.split_whitespace().collect();
    if words.first() != Some(&"info")
        || words.contains(&"lowerbound")
        || words.contains(&"upperbound")
    {
        return None;
    }
    let at = |key| words.iter().position(|&w| w == key);
    let depth = words.get(at("depth")? + 1)?.parse().ok()?;
    let index = at("multipv")
        .and_then(|i| words.get(i + 1)?.parse().ok())
        .unwrap_or(1);
    if !(1..=5).contains(&index) {
        return None;
    }
    let score_at = at("score")?;
    let n: i32 = words.get(score_at + 2)?.parse().ok()?;
    let evaluation = match *words.get(score_at + 1)? {
        "cp" => Score::Cp(n.clamp(-100_000, 100_000)),
        "mate" if n == 0 => Score::Terminal(-1),
        "mate" => Score::Mate(n.clamp(-9999, 9999)),
        _ => return None,
    }
    .red_relative(board.turn);
    let mv = words.get(at("pv")? + 1)?.parse().ok()?;
    if !board.is_legal(mv) {
        return None;
    }
    Some(Variation {
        mv,
        depth,
        index,
        score: evaluation.value(),
        evaluation,
    })
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
struct Process {
    child: std::process::Child,
    input: std::process::ChildStdin,
    rx: Receiver<String>,
}
#[cfg(not(any(target_os = "ios", target_os = "android")))]
impl Process {
    fn open(path: &Path) -> Result<Self, String> {
        use std::process::{Command, Stdio};
        let mut cmd = Command::new(path);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = path.parent() {
            cmd.current_dir(dir);
        }
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let mut child = cmd.spawn().map_err(|e| format!("启动引擎失败：{e}"))?;
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stderr);
                loop {
                    let mut bytes = Vec::new();
                    match reader.by_ref().take(4097).read_until(b'\n', &mut bytes) {
                        Ok(0) | Err(_) => break,
                        Ok(_) if bytes.len() > 4096 => break,
                        Ok(_) => crate::diagnostics::record(
                            "engine-stderr",
                            &String::from_utf8_lossy(&bytes),
                        ),
                    }
                }
            });
        }
        let input = child.stdin.take().ok_or("引擎输入不可用")?;
        let output = child.stdout.take().ok_or("引擎输出不可用")?;
        let (tx, rx) = mpsc::sync_channel(256);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut bytes = Vec::new();
                match reader.by_ref().take(65_537).read_until(b'\n', &mut bytes) {
                    Ok(0) | Err(_) => break,
                    Ok(_) if bytes.len() > 65_536 => break,
                    Ok(_) => {
                        if tx
                            .send(String::from_utf8_lossy(&bytes).trim_end().to_string())
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        });
        Ok(Self { child, input, rx })
    }
}
#[cfg(not(any(target_os = "ios", target_os = "android")))]
impl Transport for Process {
    fn send(&mut self, line: &str) -> Result<(), String> {
        writeln!(self.input, "{line}")
            .and_then(|()| self.input.flush())
            .map_err(|e| format!("引擎写入失败：{e}"))
    }
    fn receive(&mut self, timeout: Duration) -> Result<Option<String>, String> {
        match self.rx.recv_timeout(timeout) {
            Ok(line) => Ok(Some(line)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(_) => Err("引擎已退出".into()),
        }
    }
}
#[cfg(not(any(target_os = "ios", target_os = "android")))]
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(any(target_os = "ios", target_os = "android"))]
mod mobile {
    use super::*;
    use std::{
        ffi::{CStr, CString, c_void},
        sync::{Mutex, OnceLock},
    };
    unsafe extern "C" {
        fn sf_start(line: extern "C" fn(*const u8), exit: extern "C" fn(i32)) -> i32;
        fn sf_send(line: *const u8) -> i32;
        fn sf_stop();
        fn sf_free(ptr: *mut c_void);
    }
    static OUTPUT: OnceLock<Mutex<Option<mpsc::Sender<String>>>> = OnceLock::new();
    extern "C" fn on_line(p: *const u8) {
        if p.is_null() {
            return;
        }
        // The shim allocates a NUL-terminated line; sf_free is its matching allocator.
        let s = unsafe { CStr::from_ptr(p.cast()).to_string_lossy().into_owned() };
        unsafe {
            sf_free(p.cast_mut().cast());
        }
        if let Ok(guard) = OUTPUT.get_or_init(|| Mutex::new(None)).lock() {
            if let Some(tx) = guard.as_ref() {
                let _ = tx.send(s);
            }
        }
    }
    extern "C" fn on_exit(_code: i32) {
        if let Ok(mut guard) = OUTPUT.get_or_init(|| Mutex::new(None)).lock() {
            *guard = None;
        }
    }
    pub struct Ffi {
        rx: Receiver<String>,
    }
    impl Ffi {
        pub fn open() -> Result<Self, String> {
            let (tx, rx) = mpsc::channel();
            *OUTPUT
                .get_or_init(|| Mutex::new(None))
                .lock()
                .map_err(|e| e.to_string())? = Some(tx);
            let rc = unsafe { sf_start(on_line, on_exit) };
            if rc != 0 {
                return Err("进程内引擎未能启动，可能需要重启应用".into());
            }
            Ok(Self { rx })
        }
    }
    impl Transport for Ffi {
        fn send(&mut self, line: &str) -> Result<(), String> {
            let s = CString::new(line).map_err(|e| e.to_string())?;
            if unsafe { sf_send(s.as_ptr().cast()) } == 0 {
                Ok(())
            } else {
                Err("进程内引擎已停止".into())
            }
        }
        fn receive(&mut self, timeout: Duration) -> Result<Option<String>, String> {
            match self.rx.recv_timeout(timeout) {
                Ok(s) => Ok(Some(s)),
                Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
                Err(_) => Err("进程内引擎已退出".into()),
            }
        }
    }
    impl Drop for Ffi {
        fn drop(&mut self) {
            unsafe {
                sf_stop();
            }
        }
    }
}
