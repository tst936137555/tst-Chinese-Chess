use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, atomic::AtomicU64},
    time::Duration,
};
use tst_xiangqi::{
    engine::{PositionContext, Request, Score, Session, Transport, score_loss},
    game::{EndReason, Game, Outcome},
    persistence::{Command, Response, Worker},
    rules::{Board, Side},
    storage::{SCHEMA_VERSION, Storage},
};
fn started() -> Game {
    let mut g = Game::default();
    g.play("h2e2".parse().unwrap()).unwrap();
    g
}
#[test]
fn game_identity_survives_moves_undo_reload_and_changes_on_branch() {
    let mut g = started();
    let id = g.id.clone();
    let created = g.created_at;
    g.play("h7e7".parse().unwrap()).unwrap();
    g.undo_to_player(Side::Black);
    let r = g.snapshot(false, 3, false);
    assert_eq!(r.id, id);
    assert_eq!(r.time, created);
    let loaded = r.replay().unwrap();
    assert_eq!(loaded.id, id);
    assert_eq!(loaded.revision, g.revision);
    let branch = loaded.branch_at(loaded.history.len()).unwrap();
    assert_ne!(branch.id, id);
    assert_eq!(branch.history.len(), loaded.history.len());
    assert_eq!(branch.board, loaded.board);
}
#[test]
fn finish_one_game_preserves_other_active_and_upsert_preserves_favorite() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let mut a = started();
    let b = started();
    store.save_game(a.snapshot(true, 0, false), None).unwrap();
    let d = store.save_game(b.snapshot(false, 1, false), None).unwrap();
    assert_eq!(d.drafts.len(), 1);
    a.resign(Side::Red);
    let record = a.snapshot(true, 0, false);
    let d = store.save_game(record.clone(), None).unwrap();
    assert_eq!(d.active.unwrap().id, b.id);
    assert!(d.drafts.is_empty());
    store.favorite_by_id(&a.id).unwrap();
    let d = store.save_game(record, None).unwrap();
    assert_eq!(d.archives.len(), 1);
    assert!(d.archives[0].favorite);
}
#[test]
fn stale_revision_is_rejected_without_losing_current_board() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let mut a = started();
    let stale = a.snapshot(true, 0, false);
    a.play("h7e7".parse().unwrap()).unwrap();
    store.save_game(a.snapshot(true, 0, false), None).unwrap();
    assert!(store.save_game(stale, None).is_err());
    assert_eq!(store.load().unwrap().active.unwrap().history.len(), 2);
}
#[test]
fn drafts_can_be_reactivated_and_old_window_only_updates_its_draft() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let mut a = started();
    let b = started();
    store.save_game(a.snapshot(true, 0, false), None).unwrap();
    store.save_game(b.snapshot(true, 0, false), None).unwrap();
    a.play("h7e7".parse().unwrap()).unwrap();
    let d = store.save_game(a.snapshot(true, 0, false), None).unwrap();
    assert_eq!(d.active.unwrap().id, b.id);
    assert_eq!(d.drafts[0].history.len(), 2);
    let d = store.activate(&a.id).unwrap();
    assert_eq!(d.active.unwrap().id, a.id);
    assert_eq!(d.drafts[0].id, b.id);
}
#[test]
fn schema_guard_unknown_fields_backup_and_explicit_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    let store = Storage::new(path.clone());
    std::fs::write(&path, "{}").unwrap();
    assert!(store.update(|_| Ok(())).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
    std::fs::write(
        &path,
        r#"{"version":999,"settings":{},"active":null,"archives":[],"future":1}"#,
    )
    .unwrap();
    assert!(store.update(|_| Ok(())).is_err());
    assert!(store.restore_backup().is_err());
    assert!(std::fs::read_to_string(&path).unwrap().contains("999"));
    std::fs::remove_file(&path).unwrap();
    store
        .save_game(started().snapshot(true, 0, false), None)
        .unwrap();
    store
        .update(|d| {
            d.extra
                .insert("extension".into(), serde_json::json!("keep"));
            Ok(())
        })
        .unwrap();
    let d = store.update(|_| Ok(())).unwrap();
    assert_eq!(d.version, SCHEMA_VERSION);
    assert_eq!(d.extra["extension"], "keep");
    std::fs::write(&path, "{broken").unwrap();
    let d = store.restore_backup().unwrap();
    assert!(d.active.is_some());
    assert!(path.with_extension("damaged.json").is_file());
}
#[test]
fn legacy_import_is_transactional_and_future_result_cannot_override_rules() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let legacy = dir.path().join("old.json");
    std::fs::write(
        &legacy,
        r#"[{"time":1,"result":"draw","history":[{"uci":"h2e2"}]}]"#,
    )
    .unwrap();
    assert_eq!(store.import_legacy(&legacy).unwrap().1, 1);
    assert_eq!(store.import_legacy(&legacy).unwrap().1, 0);
    assert!(legacy.is_file());
    let mut zero = Game::default().snapshot(true, 0, false);
    zero.result = Outcome::BlackWin;
    assert!(zero.replay().is_err());
    let mut forged = started().snapshot(true, 0, false);
    forged.result = Outcome::BlackWin;
    assert!(forged.replay().is_err());
    let mut mate = Game::new("3k5/2R1R4/9/9/9/9/9/9/9/4K4 b - - 0 1".parse().unwrap())
        .snapshot(true, 0, false);
    mate.result = Outcome::BlackWin;
    assert!(mate.replay().is_err());
}
#[test]
fn resignation_and_evaluation_roundtrip_with_reasons() {
    let mut g = Game::default();
    g.resign(Side::Black);
    let r = g.snapshot(true, 0, false);
    let loaded = r.replay().unwrap();
    assert_eq!(loaded.outcome, Outcome::RedWin);
    assert_eq!(
        loaded.end_reason,
        Some(EndReason::Resignation { side: Side::Black })
    );
    let mut g = started();
    g.settle(-800, 12, 3000);
    assert_eq!(
        g.snapshot(true, 0, false).replay().unwrap().end_reason,
        g.end_reason
    );
}
#[test]
fn malformed_piece_counts_positions_and_oversized_input_are_rejected() {
    for fen in [
        "RRRRkRRRR/RRRRRRRRR/RRRRRRRRR/RRRRRRRRR/RRRRRRRRR/RRRRRRRRR/RRRRRRRRR/RRRRRRRRR/RRRRRRRRR/RRRRKRRRR w - - 0 1",
        "4k4/9/9/9/9/9/9/4P4/9/4K4 w - - 0 1",
        "4k4/9/9/9/9/9/9/9/3A5/4K4 w - - 0 1",
    ] {
        assert!(fen.parse::<Board>().is_err(), "{fen}");
    }
    assert!("9".repeat(300).parse::<Board>().is_err());
    let mut r = started().snapshot(true, 0, false);
    r.history.resize(4097, r.history[0].clone());
    assert!(r.replay().is_err());
}
struct Fake {
    lines: VecDeque<String>,
    sent: Arc<Mutex<Vec<String>>>,
}
impl Transport for Fake {
    fn send(&mut self, s: &str) -> Result<(), String> {
        self.sent.lock().unwrap().push(s.into());
        Ok(())
    }
    fn receive(&mut self, _: Duration) -> Result<Option<String>, String> {
        self.lines.pop_front().map(Some).ok_or("EOF".into())
    }
}
fn session(lines: &[&str]) -> (Session, Arc<Mutex<Vec<String>>>) {
    let sent = Arc::new(Mutex::new(Vec::new()));
    (
        Session::with_transport(Box::new(Fake {
            lines: lines.iter().map(|s| s.to_string()).collect(),
            sent: sent.clone(),
        })),
        sent,
    )
}
fn request() -> Request {
    Request {
        id: 1,
        board: Board::default(),
        context: None,
        depth: 12,
        ms: 20,
        pv: 2,
    }
}
#[test]
fn engine_receives_verified_history_and_newgame_boundaries() {
    let g = started();
    let (mut s, sent) = session(&[
        "readyok",
        "info depth 4 score cp 20 pv h7e7",
        "bestmove h7e7",
    ]);
    let mut req = request();
    req.board = g.board.clone();
    req.context = Some(PositionContext {
        start: g.start.clone(),
        moves: g.history.iter().map(|e| e.mv).collect(),
        game_id: g.id.clone(),
    });
    s.search(&req, &AtomicU64::new(1)).unwrap();
    let sent = sent.lock().unwrap();
    assert!(sent.iter().any(|s| s == "ucinewgame"));
    assert!(sent.iter().any(|s| s.ends_with(" moves h2e2")));
    drop(sent);
    req.board = Board::default();
    assert!(
        session(&["readyok"])
            .0
            .search(&req, &AtomicU64::new(1))
            .is_err()
    );
}
#[test]
fn multipv_never_combines_partial_iterations_or_duplicates() {
    let (mut s, _) = session(&[
        "readyok",
        "info depth 8 multipv 1 score cp 50 pv h2e2",
        "info depth 8 multipv 2 score cp 40 pv b2e2",
        "info depth 9 multipv 1 score cp 60 pv b2e2",
        "bestmove b2e2",
    ]);
    let a = s.search(&request(), &AtomicU64::new(1)).unwrap();
    assert_eq!(a.variations.len(), 1);
    assert_eq!(a.variations[0].depth, 9);
    let (mut s, _) = session(&[
        "readyok",
        "info depth 8 multipv 1 score cp 50 pv h2e2",
        "info depth 8 multipv 2 score cp 40 pv b2e2",
        "info depth 9 multipv 1 score cp 60 pv h2e2",
        "bestmove h2e2",
    ]);
    let a = s.search(&request(), &AtomicU64::new(1)).unwrap();
    assert_eq!(a.variations.len(), 2);
    assert!(a.variations.iter().all(|v| v.depth == 8));
    assert_ne!(a.variations[0].mv, a.variations[1].mv);
}
#[test]
fn mate_distances_are_not_centipawn_losses() {
    assert_eq!(score_loss(Score::Mate(2), Score::Mate(8), Side::Red), None);
    assert_eq!(
        score_loss(Score::Cp(150), Score::Cp(20), Side::Red),
        Some(130)
    );
    assert!(Score::Mate(4).label().contains("杀棋"));
}
#[test]
fn async_persistence_flushes_in_order_and_preserves_errors() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let worker = Worker::new(store.clone());
    let mut g = started();
    worker
        .send(Command::save(g.snapshot(true, 0, false), None))
        .unwrap();
    g.resign(Side::Red);
    worker
        .send(Command::save(g.snapshot(true, 0, false), None))
        .unwrap();
    worker.send(Command::Flush).unwrap();
    let mut flushed = false;
    for _ in 0..3 {
        let e = worker.rx.recv_timeout(Duration::from_secs(20)).unwrap();
        if let Response::Flushed = e.result.unwrap() {
            flushed = true;
        }
    }
    assert!(flushed);
    drop(worker);
    assert_eq!(store.load().unwrap().archives.len(), 1);
}
#[test]
fn nnue_replacement_validates_bytes_and_preserves_previous_asset() {
    use sha2::{Digest, Sha256};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("net.nnue");
    std::fs::write(&path, b"old").unwrap();
    assert!(tst_xiangqi::platform::install_nnue(&path, b"new", "invalid").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"old");
    let hash = format!("{:x}", Sha256::digest(b"new"));
    tst_xiangqi::platform::install_nnue(&path, b"new", &hash).unwrap();
    assert!(tst_xiangqi::platform::nnue_matches(&path, &hash));
    assert!(!tst_xiangqi::platform::nnue_matches(&path, "wrong"));
}
#[test]
fn randomized_play_undo_restores_fen_and_repetition_state() {
    let mut g = Game::default();
    let mut seed = 0x12345678u64;
    for _ in 0..30 {
        if g.outcome != Outcome::Playing {
            break;
        }
        let before = g.clone();
        let moves = g.board.legal_moves();
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        g.play(moves[seed as usize % moves.len()]).unwrap();
        g.undo_to_player(before.board.turn);
        assert_eq!(g.board, before.board);
        assert_eq!(g.repetition_count(), before.repetition_count());
        g.play(moves[(seed >> 16) as usize % moves.len()]).unwrap();
    }
}
#[test]
fn review_cache_survives_worker_restart_with_typed_scores() {
    if tst_xiangqi::platform::nnue_path().is_err() || tst_xiangqi::platform::engine_path().is_err()
    {
        return;
    }
    use tst_xiangqi::review::CachedPosition;
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    {
        let worker = Worker::new(store.clone());
        worker
            .send(Command::CachePut {
                key: "history-sensitive-key".into(),
                entries: vec![Some(CachedPosition {
                    score: Score::Mate(4),
                    best: Some("h2e2".into()),
                    depth: 12,
                    details: None,
                })],
            })
            .unwrap();
        worker.send(Command::Flush).unwrap();
        for _ in 0..2 {
            worker
                .rx
                .recv_timeout(Duration::from_secs(20))
                .unwrap()
                .result
                .unwrap();
        }
    }
    let worker = Worker::new(store);
    worker
        .send(Command::CacheLoad {
            key: "history-sensitive-key".into(),
        })
        .unwrap();
    let event = worker.rx.recv_timeout(Duration::from_secs(20)).unwrap();
    if let Response::Cache {
        review: Some(r), ..
    } = event.result.unwrap()
    {
        assert_eq!(r.entries[0].as_ref().unwrap().score, Score::Mate(4));
        assert_eq!(r.entries[0].as_ref().unwrap().best.as_deref(), Some("h2e2"));
    } else {
        panic!("cache was not restored");
    }
}
#[test]
fn mate_zero_is_loss_for_side_to_move_even_when_black() {
    let mut board = Board::default();
    board.turn = Side::Black;
    let v = tst_xiangqi::engine::parse_info("info depth 4 score mate 0 pv h7e7", &board).unwrap();
    assert_eq!(v.evaluation, Score::Terminal(1));
    assert_eq!(v.score, 100_000);
}
#[test]
fn worker_returns_retryable_operation_and_preserves_future_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    let content = r#"{"version":999,"future":"keep"}"#;
    std::fs::write(&path, content).unwrap();
    let worker = Worker::new(Storage::new(path.clone()));
    worker
        .send(Command::save(started().snapshot(true, 0, false), None))
        .unwrap();
    let event = worker.rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(event.result.is_err());
    assert!(matches!(event.retry, Some(Command::Save(..))));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
}
#[test]
fn retried_favorite_operation_is_applied_once() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    let mut g = started();
    g.resign(Side::Black);
    store.save_game(g.snapshot(true, 0, false), None).unwrap();
    assert!(store.favorite_once(&g.id, "operation-1").unwrap().archives[0].favorite);
    assert!(store.favorite_once(&g.id, "operation-1").unwrap().archives[0].favorite);
    assert!(!store.favorite_once(&g.id, "operation-2").unwrap().archives[0].favorite);
}
#[test]
fn backup_restore_preserves_oversized_and_previous_damaged_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    let store = Storage::new(path.clone());
    store
        .save_game(started().snapshot(true, 0, false), None)
        .unwrap();
    store.update(|_| Ok(())).unwrap();
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(33 * 1024 * 1024).unwrap();
    drop(file);
    assert!(store.restore_backup().is_err());
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 33 * 1024 * 1024);
    std::fs::write(&path, b"broken-a").unwrap();
    store.restore_backup().unwrap();
    std::fs::write(&path, b"broken-b").unwrap();
    store.restore_backup().unwrap();
    assert_eq!(
        std::fs::read(path.with_extension("damaged.json")).unwrap(),
        b"broken-a"
    );
    assert!(
        std::fs::read_dir(dir.path()).unwrap().flatten().any(|e| e
            .file_name()
            .to_string_lossy()
            .contains("damaged-")
            && std::fs::read(e.path()).is_ok_and(|v| v == b"broken-b"))
    );
}
