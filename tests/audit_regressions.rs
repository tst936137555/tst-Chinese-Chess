use std::time::Duration;
use tst_xiangqi::{
    engine::{Score, score_loss},
    game::{EvaluationEvidence, Game},
    persistence::{Command, Response, Worker},
    review::{self, CachedPosition},
    rules::{Board, Side},
    session::{Recovery, SaveState, TaskKind},
    storage::Storage,
};
fn play(g: &mut Game, m: &str) {
    g.play(m.parse().unwrap()).unwrap();
}
fn event(w: &Worker) -> Response {
    w.rx.recv_timeout(Duration::from_secs(30))
        .unwrap()
        .result
        .unwrap()
}
fn flush(w: &Worker) {
    w.send(Command::Flush).unwrap();
    loop {
        if matches!(event(w), Response::Flushed) {
            break;
        }
    }
}
#[test]
fn divergent_newer_revision_cannot_overwrite_and_undo_remains_safe() {
    let d = tempfile::tempdir().unwrap();
    let s = Storage::new(d.path().join("state.json"));
    let base = Game::default();
    let mut a = base.clone();
    play(&mut a, "h2e2");
    s.save_game(a.snapshot(true, 0, false), None).unwrap();
    let mut b = base;
    play(&mut b, "b0c2");
    play(&mut b, "b9c7");
    assert!(
        s.save_game(b.snapshot(true, 0, false), None)
            .unwrap_err_or_string()
            .contains("[conflict]")
    );
    let mut loaded = s.load().unwrap().active.unwrap().replay().unwrap();
    play(&mut loaded, "h7e7");
    loaded.undo_to_player(Side::Red);
    s.save_game(loaded.snapshot(true, 0, false), None).unwrap();
    assert!(s.load().unwrap().active.unwrap().history.is_empty());
}
trait ErrorString {
    fn unwrap_err_or_string(self) -> String;
}
impl<T> ErrorString for Result<T, String> {
    fn unwrap_err_or_string(self) -> String {
        match self {
            Ok(_) => panic!("expected conflict"),
            Err(e) => e,
        }
    }
}
#[test]
fn conflict_ack_requires_same_content_or_proven_descendant() {
    let base = Game::default();
    let mut a = base.clone();
    play(&mut a, "h2e2");
    let mut b = base;
    play(&mut b, "b0c2");
    let failed = b.snapshot(true, 0, false);
    let mut state = SaveState::default();
    state.failed(Command::save(failed.clone(), None));
    let mut data = tst_xiangqi::storage::Data {
        active: Some(a.snapshot(true, 0, false)),
        ..Default::default()
    };
    state.acknowledge(&data);
    assert_eq!(state.failed.len(), 1);
    play(&mut a, "h7e7");
    data.active = Some(a.snapshot(true, 0, false));
    state.acknowledge(&data);
    assert_eq!(state.failed.len(), 1);
    play(&mut b, "b9c7");
    data.active = Some(b.snapshot(true, 0, false));
    state.acknowledge(&data);
    assert!(state.failed.is_empty());
}
#[test]
fn failed_save_then_later_descendant_retries_without_losing_intermediate_moves() {
    let d = tempfile::tempdir().unwrap();
    let s = Storage::new(d.path().join("state.json"));
    let mut g = Game::default();
    s.save_game(g.snapshot(true, 0, false), None).unwrap();
    play(&mut g, "h2e2");
    play(&mut g, "h7e7");
    let r = g.snapshot(true, 0, false);
    s.save_game(r.clone(), None).unwrap();
    s.save_game(r, None).unwrap();
    assert_eq!(s.load().unwrap().active.unwrap().history.len(), 2);
}
#[test]
fn rule_ending_scores_cover_long_check_draw_and_human_endings() {
    let mut g = Game::new(
        "5k3/9/9/9/9/9/9/9/3r5/4K4 b - - 0 1"
            .parse::<Board>()
            .unwrap(),
    );
    for m in [
        "d1e1", "e0d0", "e1d1", "d0e0", "d1e1", "e0d0", "e1d1", "d0e0",
    ] {
        play(&mut g, m);
    }
    assert_eq!(review::rule_score(&g, 8), Some(Score::Terminal(1)));
    assert_eq!(review::rule_score(&g, 7), None);
    let mut g = Game::default();
    g.resign(Side::Red);
    assert_eq!(review::rule_score(&g, 0), None);
    let g = Game::new(
        tst_xiangqi::rules::START_FEN
            .replace("0 1", "120 80")
            .parse()
            .unwrap(),
    );
    assert_eq!(review::rule_score(&g, 0), Some(Score::Cp(0)));
}
#[test]
fn mate_improvements_never_receive_a_loss_for_either_side() {
    for (a, b, side) in [
        (Score::Mate(-3), Score::Mate(3), Side::Red),
        (Score::Mate(3), Score::Mate(-3), Side::Black),
        (Score::Cp(0), Score::Mate(3), Side::Red),
        (Score::Cp(0), Score::Terminal(-1), Side::Black),
    ] {
        assert_eq!(score_loss(a, b, side), Some(0));
    }
    assert_eq!(
        score_loss(Score::Mate(3), Score::Mate(-3), Side::Red),
        Some(100000)
    );
    assert_eq!(score_loss(Score::Mate(3), Score::Mate(7), Side::Red), None);
}
#[test]
fn typed_settlement_evidence_roundtrips_and_rejects_forged_depth() {
    let mut g = Game::default();
    g.settle_evaluated(EvaluationEvidence {
        score: Score::Mate(3),
        requested_depth: 12,
        reached_depth: 5,
        budget_ms: 3000,
        elapsed_ms: 41,
        assets: "test-asset-digest".into(),
    });
    let mut r = g.snapshot(true, 0, false);
    let loaded = r.replay().unwrap();
    assert_eq!(g.end_reason, loaded.end_reason);
    if let Some(tst_xiangqi::game::EndReason::Evaluation {
        evidence: Some(e), ..
    }) = &mut r.end_reason
    {
        e.reached_depth = 64;
    }
    assert!(r.replay().is_err());
}
#[test]
fn legacy_collisions_preserve_distinct_games_and_repeat_import_is_idempotent() {
    let d = tempfile::tempdir().unwrap();
    let s = Storage::new(d.path().join("state.json"));
    let old = d.path().join("中文 旧棋谱.json");
    std::fs::write(&old,r#"[{"time":42,"userRed":true,"result":"draw","history":[{"uci":"h2e2"}]},{"time":42,"userRed":true,"result":"draw","history":[{"uci":"b0c2"}]}]"#).unwrap();
    assert_eq!(s.import_legacy(&old).unwrap().1, 2);
    assert_eq!(s.import_legacy(&old).unwrap().1, 0);
    assert_eq!(s.load().unwrap().archives.len(), 2);
}
#[test]
fn every_task_survives_repeated_lifecycle_pause_and_resume() {
    for kind in [
        TaskKind::Move,
        TaskKind::Hint,
        TaskKind::Review,
        TaskKind::End,
    ] {
        let mut r = Recovery::default();
        r.pause(Some(kind));
        r.pause(None);
        assert_eq!(r.resume(true), None);
        assert_eq!(r.resume(false), Some(kind));
        assert_eq!(r.resume(false), None);
    }
}
#[test]
fn legacy_source_roundtrips_independently_of_id_prefix() {
    let raw = r#"{"time":42,"result":"draw","history":[{"uci":"h2e2"}]}"#;
    let r: tst_xiangqi::game::Record = serde_json::from_str(raw).unwrap();
    let game = r.replay().unwrap();
    assert_eq!(game.source, tst_xiangqi::game::RecordSource::Legacy);
    assert!(game.snapshot(true, 0, false).replay().is_ok());
    let mut forged = Game::default().snapshot(true, 0, false);
    forged.id = "legacy-forged".into();
    forged.result = tst_xiangqi::game::Outcome::RedWin;
    assert!(forged.replay().is_err());
}
#[test]
fn per_ply_randomness_survives_restore_and_varies_between_seeds() {
    use tst_xiangqi::engine::{Analysis, Variation, choose_variation};
    let a = Analysis {
        best: "h2e2".parse().unwrap(),
        variations: [("h2e2", 90), ("b2e2", 80)]
            .into_iter()
            .enumerate()
            .map(|(i, (m, score))| Variation {
                mv: m.parse().unwrap(),
                score,
                depth: 3,
                index: i + 1,
                evaluation: Score::Cp(score),
            })
            .collect(),
        elapsed_ms: 1,
        requested_depth: 3,
        budget_ms: 1000,
        assets: "test".into(),
    };
    let mut game = Game::default();
    play(&mut game, "b0c2");
    let restored = game.snapshot(true, 0, false).replay().unwrap();
    assert_eq!(
        choose_variation(&a, Side::Red, 300, game.random_seed, game.history.len()),
        choose_variation(
            &a,
            Side::Red,
            300,
            restored.random_seed,
            restored.history.len()
        )
    );
    assert!((0..100).any(|seed| choose_variation(&a, Side::Red, 300, seed, 1) != a.best));
}
fn put(key: &str, cp: i32, depth: u32) -> Command {
    Command::CachePut {
        key: key.into(),
        entries: vec![Some(CachedPosition {
            score: Score::Cp(cp),
            best: Some("h2e2".into()),
            depth,
        })],
    }
}
#[test]
fn cache_timer_persists_final_update_without_another_command() {
    if tst_xiangqi::persistence::asset_fingerprint().is_err() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let w = Worker::new(Storage::new(d.path().join("state.json")));
    w.send(put("idle", 22, 8)).unwrap();
    assert!(matches!(event(&w), Response::SavedCache));
    loop {
        if matches!(event(&w), Response::CachePersisted) {
            break;
        }
    }
    assert!(
        std::fs::read_to_string(d.path().join("analysis.json"))
            .unwrap()
            .contains("\"value\":22")
    );
}
#[test]
fn independent_cache_workers_merge_and_never_replace_deeper_entries() {
    if tst_xiangqi::persistence::asset_fingerprint().is_err() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let s = Storage::new(d.path().join("state.json"));
    let a = Worker::new(s.clone());
    let b = Worker::new(s);
    for w in [&a, &b] {
        w.send(Command::CacheLoad {
            key: "missing".into(),
        })
        .unwrap();
        assert!(matches!(event(w), Response::Cache { .. }));
    }
    a.send(put("a", 30, 12)).unwrap();
    event(&a);
    flush(&a);
    b.send(put("b", 40, 8)).unwrap();
    event(&b);
    b.send(put("a", 1, 3)).unwrap();
    event(&b);
    flush(&b);
    let text = std::fs::read_to_string(d.path().join("analysis.json")).unwrap();
    assert!(text.contains("\"value\":30"));
    assert!(text.contains("\"value\":40"));
    assert!(!text.contains("\"value\":1"));
}
#[test]
fn cache_failure_is_reported_but_flush_still_completes() {
    if tst_xiangqi::persistence::asset_fingerprint().is_err() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("analysis.json")).unwrap();
    let w = Worker::new(Storage::new(d.path().join("state.json")));
    w.send(put("failure", 1, 3)).unwrap();
    event(&w);
    w.send(Command::Flush).unwrap();
    assert!(matches!(event(&w), Response::CacheWarning(_)));
    assert!(matches!(event(&w), Response::Flushed));
}
#[test]
fn draft_names_survive_play_and_clearing_drafts_keeps_active_game() {
    let d = tempfile::tempdir().unwrap();
    let s = Storage::new(d.path().join("state.json"));
    let mut a = Game::default();
    let b = Game::default();
    s.save_game(a.snapshot(true, 0, false), None).unwrap();
    s.save_game(b.snapshot(true, 0, false), None).unwrap();
    let w = Worker::new(s.clone());
    w.send(Command::Rename(a.id.clone(), "残局练习".into()))
        .unwrap();
    event(&w);
    play(&mut a, "h2e2");
    let data = s.save_game(a.snapshot(true, 0, false), None).unwrap();
    assert_eq!(data.drafts[0].title, "残局练习");
    w.send(Command::ClearDrafts).unwrap();
    event(&w);
    let data = s.load().unwrap();
    assert!(data.drafts.is_empty());
    assert_eq!(data.active.unwrap().id, b.id);
}
#[test]
fn full_draft_capacity_is_transactional_and_recoverable() {
    let d = tempfile::tempdir().unwrap();
    let s = Storage::new(d.path().join("state.json"));
    for _ in 0..51 {
        s.save_game(Game::default().snapshot(true, 0, false), None)
            .unwrap();
    }
    let before = std::fs::read(&s.path).unwrap();
    assert!(
        s.save_game(Game::default().snapshot(true, 0, false), None)
            .is_err()
    );
    assert_eq!(std::fs::read(&s.path).unwrap(), before);
    let id = s.load().unwrap().drafts[0].id.clone();
    s.remove_by_id(&id).unwrap();
    s.save_game(Game::default().snapshot(true, 0, false), None)
        .unwrap();
}
