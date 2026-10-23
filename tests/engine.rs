use std::{collections::VecDeque, path::Path, sync::atomic::AtomicU64, time::Duration};
use tst_xiangqi::{
    engine::{Request, Session, Transport, parse_info},
    rules::{Board, Side},
};
struct Fake {
    lines: VecDeque<String>,
    sent: Vec<String>,
}
impl Transport for Fake {
    fn send(&mut self, s: &str) -> Result<(), String> {
        self.sent.push(s.into());
        Ok(())
    }
    fn receive(&mut self, _: Duration) -> Result<Option<String>, String> {
        self.lines.pop_front().map(Some).ok_or("fake EOF".into())
    }
}
fn session(lines: &[&str]) -> Session {
    Session::with_transport(Box::new(Fake {
        lines: lines.iter().map(|s| s.to_string()).collect(),
        sent: vec![],
    }))
}
#[test]
fn protocol_handshake_and_search_validate_real_moves() {
    let token = AtomicU64::new(1);
    let mut s = session(&[
        "id name Fake",
        "uciok",
        "readyok",
        "readyok",
        "info depth 4 score cp 33 pv h2e2",
        "bestmove h2e2",
    ]);
    s.handshake(Path::new("test.nnue"), &token, 1).unwrap();
    let req = Request {
        context: None,
        id: 1,
        board: Board::default(),
        depth: 4,
        ms: 10,
        pv: 1,
    };
    let a = s.search(&req, &token).unwrap();
    assert_eq!(a.best.to_string(), "h2e2");
    assert_eq!(a.variations[0].score, 33);
}
#[test]
fn illegal_no_score_bound_only_and_stale_results_are_errors() {
    let token = AtomicU64::new(1);
    let req = Request {
        context: None,
        id: 1,
        board: Board::default(),
        depth: 4,
        ms: 10,
        pv: 1,
    };
    for lines in [
        vec![
            "readyok",
            "info depth 4 score cp 1 pv a0a9",
            "bestmove a0a9",
        ],
        vec!["readyok", "bestmove h2e2"],
        vec![
            "readyok",
            "info depth 4 score cp 1 lowerbound pv h2e2",
            "bestmove h2e2",
        ],
    ] {
        assert!(session(&lines).search(&req, &token).is_err());
    }
    assert!(
        session(&["uciok"])
            .handshake(Path::new("test.nnue"), &AtomicU64::new(2), 1)
            .is_err()
    );
}
#[test]
fn scores_are_red_relative_and_malformed_input_is_ignored() {
    let mut b = Board::default();
    b.turn = Side::Black;
    let v = parse_info("info depth 5 multipv 2 score cp 50 pv h7e7", &b).unwrap();
    assert_eq!(v.score, -50);
    assert_eq!(v.index, 2);
    assert!(parse_info("info depth 1 score cp 12 pv j2e2", &b).is_none());
    assert!(parse_info("info depth 1 score cp 12 lowerbound pv h7e7", &b).is_none());
}

#[test]
fn an_in_flight_search_is_cancelled_without_waiting_for_the_watchdog() {
    struct Hanging {
        ready: bool,
    }
    impl Transport for Hanging {
        fn send(&mut self, _: &str) -> Result<(), String> {
            Ok(())
        }
        fn receive(&mut self, wait: Duration) -> Result<Option<String>, String> {
            if !self.ready {
                self.ready = true;
                return Ok(Some("readyok".into()));
            }
            std::thread::sleep(wait);
            Ok(None)
        }
    }
    let token = std::sync::Arc::new(AtomicU64::new(7));
    let worker_token = token.clone();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        worker_token.store(8, std::sync::atomic::Ordering::Release);
    });
    let mut session = Session::with_transport(Box::new(Hanging { ready: false }));
    let start = std::time::Instant::now();
    let result = session.search(
        &Request {
            context: None,
            id: 7,
            board: Board::default(),
            depth: 12,
            ms: 3000,
            pv: 1,
        },
        &token,
    );
    assert_eq!(result.unwrap_err(), "搜索已取消");
    assert!(start.elapsed() < Duration::from_secs(1));
    worker.join().unwrap();
}
#[test]
fn real_engine_if_installed() {
    if tst_xiangqi::platform::engine_path().is_err() || tst_xiangqi::platform::nnue_path().is_err()
    {
        assert!(
            std::env::var_os("XIANGQI_REQUIRE_ENGINE").is_none(),
            "CI requires real Pikafish assets"
        );
        eprintln!("real Pikafish assets unavailable; protocol tests still run");
        return;
    }
    let e = tst_xiangqi::engine::Engine::new();
    e.request(Board::default(), 3, 1000, 2).unwrap();
    let reply = e.rx.recv_timeout(Duration::from_secs(30)).unwrap();
    let a = reply.result.unwrap();
    assert!(Board::default().is_legal(a.best));
    assert!(!a.variations.is_empty());
}
