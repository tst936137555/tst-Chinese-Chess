use tst_xiangqi::{
    engine::{MoveAssessment, Score, assess_move},
    game::Game,
    review::{ReviewSession, marked_chart_samples},
    rules::Side,
};

fn review() -> ReviewSession {
    let mut game = Game::default();
    for mv in ["h2e2", "h7e7", "b0c2", "b9c7", "a3a4", "a6a5"] {
        game.play(mv.parse().unwrap()).unwrap();
    }
    ReviewSession::new(game, true)
}

#[test]
fn newly_allowing_mate_is_a_blunder_for_either_mover_even_with_an_old_best() {
    for (ply, side) in [(1, Side::Red), (2, Side::Black)] {
        for after in [Score::Mate(-3), Score::Terminal(-1)] {
            let mut r = review();
            r.scores[ply - 1] = Some(Score::Cp(-400).red_relative(side));
            r.scores[ply] = Some(after.red_relative(side));
            r.bests[ply - 1] = Some(r.game.history[ply - 1].mv);
            let assessment = r.assessment(ply).unwrap();
            assert_eq!(assessment, MoveAssessment::AllowsMate);
            assert_eq!(assessment.label(), "错招");
            assert_eq!(assessment.badge(), "错");
            assert_eq!(r.mistake_plies(), [ply]);
        }
        // An existing forced loss is not a new mistake on every subsequent move.
        let continued = assess_move(
            Score::Mate(-5).red_relative(side),
            Score::Mate(-3).red_relative(side),
            side,
        );
        assert_eq!(continued, MoveAssessment::MateContinuation);
        assert!(!continued.is_mistake());
    }
}

#[test]
fn chart_markers_follow_move_quality_and_never_guess_missing_scores() {
    let mut r = review();
    r.scores = [0, 400, 700, 100, -300, -549, -299]
        .map(|cp| Some(Score::Cp(cp)))
        .to_vec();
    assert_eq!(r.mistake_plies(), [2, 3, 6]);
    assert_eq!(r.assessment(2).unwrap().badge(), "差");
    assert_eq!(r.assessment(3).unwrap().badge(), "错");
    assert!(!r.assessment(5).unwrap().is_mistake());
    r.bests[1] = Some(r.game.history[1].mv);
    assert_eq!(r.mistake_plies(), [3, 6]);
    r.scores[3] = None;
    assert_eq!(r.mistake_plies(), [6]);
    assert!(r.assessment(0).is_none());
    assert!(r.assessment(7).is_none());
    assert!(MoveAssessment::MissedMate.is_mistake());
    assert!(!MoveAssessment::FoundMate.is_mistake());
    assert!(!MoveAssessment::EscapedMate.is_mistake());
}

#[test]
fn downsampled_chart_keeps_markers_and_analysis_gaps_at_exact_plies() {
    let mut scores = (0..4097).map(|i| Some(Score::Cp(i))).collect::<Vec<_>>();
    scores[600] = None;
    let markers = [123, 127, 599, 601];
    let points = marked_chart_samples(&scores, 100, &markers);
    for ply in markers.into_iter().chain([600]) {
        assert!(points.contains(&(ply, scores[ply])));
    }
    assert!(points.len() < 500);
    assert!(points.windows(2).all(|p| p[0].0 < p[1].0));
}
