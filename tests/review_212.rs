use tst_xiangqi::{
    engine::{MoveAssessment, Score},
    game::Game,
    review::{ANALYSIS_LIMITS, AnalysisDetails, ReviewSession},
};

fn review() -> ReviewSession {
    let mut game = Game::default();
    for word in ["h2e2", "h7e7", "b0c2", "b9c7", "a3a4", "a6a5"] {
        game.play(word.parse().unwrap()).unwrap();
    }
    ReviewSession::new(game, true)
}

#[test]
fn review_unlocks_only_after_every_position_has_finished_analysis() {
    let mut r = review();
    assert!(!r.analysis_complete());
    r.cache_loading = false;
    r.scores.fill(Some(Score::Cp(0)));
    // Old scores alone cannot unlock the UI before the current analysis.
    assert!(!r.analysis_complete());
    r.attempted.fill(true);
    r.attempted[3] = false;
    assert!(!r.analysis_complete());
    r.attempted[3] = true;
    assert!(r.analysis_complete());
    r.scores[4] = None;
    assert!(!r.analysis_complete());
    r.scores[4] = Some(Score::Cp(0));
    r.cache_loading = true;
    assert!(!r.analysis_complete());
}

#[test]
fn current_cached_analysis_unlocks_without_another_engine_search() {
    let mut r = review();
    r.cache_loading = false;
    r.scores.fill(Some(Score::Cp(0)));
    r.details.fill(Some(AnalysisDetails {
        line: Vec::new(),
        elapsed_ms: 1,
        budget_ms: ANALYSIS_LIMITS.1,
        requested_depth: ANALYSIS_LIMITS.0,
    }));
    assert!(r.analysis_complete());
    r.details[2].as_mut().unwrap().budget_ms = 1;
    assert!(!r.analysis_complete());
}

#[test]
fn every_move_uses_one_of_the_five_grades_including_mate_positions() {
    for (loss, grade) in [
        (0, "优"),
        (29, "优"),
        (30, "良"),
        (99, "良"),
        (100, "平"),
        (249, "平"),
        (250, "差"),
        (599, "差"),
        (600, "错"),
    ] {
        assert_eq!(MoveAssessment::Centipawns(loss).badge(), grade);
    }
    for (assessment, grade) in [
        (MoveAssessment::MissedMate, "错"),
        (MoveAssessment::AllowsMate, "错"),
        (MoveAssessment::FoundMate, "优"),
        (MoveAssessment::EscapedMate, "优"),
        (MoveAssessment::MateContinuation, "平"),
    ] {
        assert_eq!(assessment.badge(), grade);
    }
    let mut r = review();
    r.scores = [0, -30, 70, -179, 421, 421, 421]
        .map(|cp| Some(Score::Cp(cp)))
        .to_vec();
    assert_eq!(
        (1..=6)
            .map(|ply| r.assessment(ply).unwrap().badge())
            .collect::<Vec<_>>(),
        ["良", "平", "平", "错", "优", "优"]
    );
}
