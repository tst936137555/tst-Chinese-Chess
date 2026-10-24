use super::*;
use std::collections::BTreeMap;
pub trait Transport: Send {
    fn send(&mut self, line: &str) -> Result<(), String>;
    fn receive(&mut self, timeout: Duration) -> Result<Option<String>, String>;
    fn send_until(
        &mut self,
        line: &str,
        _deadline: Instant,
        _token: &AtomicU64,
        _id: u64,
    ) -> Result<(), String> {
        self.send(line)
    }
}
pub struct Session {
    io: Box<dyn Transport>,
    game_id: Option<String>,
    assets: String,
    pub(super) failure_kind: crate::diagnostics::FailureKind,
    pub(super) deadline: Option<Instant>,
    history: crate::history::HistoryVerifier,
}
impl Session {
    pub fn with_transport(io: Box<dyn Transport>) -> Self {
        Self {
            io,
            game_id: None,
            assets: "test-transport".into(),
            failure_kind: crate::diagnostics::FailureKind::Protocol,
            deadline: None,
            history: crate::history::HistoryVerifier::default(),
        }
    }
    pub(super) fn open(
        token: &AtomicU64,
        id: u64,
        deadline: Instant,
    ) -> Result<Self, (crate::diagnostics::FailureKind, String)> {
        let asset_error = |e| (crate::diagnostics::FailureKind::Assets, e);
        let nnue = crate::platform::nnue_path().map_err(asset_error)?;
        #[cfg(any(target_os = "ios", target_os = "android"))]
        let io = mobile::Ffi::open()
            .map_err(|message| (crate::diagnostics::FailureKind::RestartRequired, message))?;
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let io = Process::open(&crate::platform::engine_path().map_err(asset_error)?)
            .map_err(asset_error)?;
        let mut session = Self::with_transport(Box::new(io));
        session.deadline = Some(deadline);
        session.assets = crate::persistence::asset_fingerprint().map_err(asset_error)?;
        session
            .handshake(&nnue, token, id)
            .map_err(|e| (session.failure_kind, e))?;
        Ok(session)
    }
    pub fn handshake(&mut self, nnue: &Path, token: &AtomicU64, id: u64) -> Result<(), String> {
        let end = self
            .deadline
            .unwrap_or_else(|| Instant::now() + Duration::from_secs(20));
        let path = nnue.to_str().ok_or("NNUE 路径编码无效")?;
        if path.contains(['\r', '\n']) {
            return Err("NNUE 路径包含换行".into());
        }
        self.send("uci", end, token, id)?;
        self.wait_for("uciok", Duration::from_secs(10), token, id)?;
        self.send("setoption name Threads value 1", end, token, id)?;
        self.send("setoption name Hash value 32", end, token, id)?;
        self.send(
            &format!("setoption name EvalFile value {path}"),
            end,
            token,
            id,
        )?;
        self.send("isready", end, token, id)?;
        self.wait_for("readyok", Duration::from_secs(10), token, id)
    }
    fn wait_for(
        &mut self,
        expected: &str,
        timeout: Duration,
        token: &AtomicU64,
        id: u64,
    ) -> Result<(), String> {
        let end = (Instant::now() + timeout)
            .min(self.deadline.unwrap_or_else(|| Instant::now() + timeout));
        loop {
            if let Some(line) = self.next(end, token, id)?
                && line.trim() == expected
            {
                return Ok(());
            }
        }
    }
    fn send(&mut self, line: &str, end: Instant, token: &AtomicU64, id: u64) -> Result<(), String> {
        self.check_deadline(end, token, id)?;
        if let Err(error) = self.io.send_until(line, end, token, id) {
            self.check_deadline(end, token, id)?;
            return Err(error);
        }
        self.check_deadline(end, token, id)
    }
    fn check_deadline(&mut self, end: Instant, token: &AtomicU64, id: u64) -> Result<(), String> {
        if crate::platform::suspended() || token.load(Ordering::Acquire) != id {
            self.failure_kind = crate::diagnostics::FailureKind::Cancelled;
            return Err("搜索已取消".into());
        }
        if Instant::now() >= end {
            self.failure_kind = crate::diagnostics::FailureKind::Timeout;
            return Err("引擎请求总时限已到".into());
        }
        Ok(())
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
        let end = self
            .deadline
            .unwrap_or_else(|| started + Duration::from_millis(req.ms.clamp(1, 60_000) + 5000));
        if let Some(ctx) = &req.context
            && self.game_id.as_ref() != Some(&ctx.game_id)
        {
            self.send("ucinewgame", end, token, req.id)?;
            self.game_id = Some(ctx.game_id.clone());
        }
        self.send(
            &format!("setoption name MultiPV value {}", req.pv.clamp(1, 5)),
            end,
            token,
            req.id,
        )?;
        self.send("isready", end, token, req.id)?;
        self.wait_for("readyok", Duration::from_secs(5), token, req.id)?;
        let position = if let Some(ctx) = &req.context {
            let checked = self
                .history
                .verify(&ctx.game_id, &ctx.start, &ctx.moves, &req.board)?;
            crate::diagnostics::measure("history_moves_verified", checked as f64);
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
        self.send(&position, end, token, req.id)?;
        self.send(
            &format!(
                "go depth {} movetime {}",
                req.depth.clamp(1, 64),
                req.ms.clamp(1, 60_000)
            ),
            end,
            token,
            req.id,
        )?;
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
    if !(1..=64).contains(&depth) {
        return None;
    }
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
