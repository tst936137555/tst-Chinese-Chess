use tst_xiangqi::{
    game::Game,
    review::{ANALYSIS_LIMITS, AnalysisDetails, ReviewSession},
    rules::Side,
};

fn review() -> ReviewSession {
    let mut game = Game::default();
    for word in ["h2e2", "h7e7", "b0c2", "b9c7", "a3a4", "a6a5", "a4a5"] {
        game.play(word.parse().unwrap()).unwrap();
    }
    let mut review = ReviewSession::new(game, true);
    for ply in 0..review.bests.len() {
        let board = review.game.at_ply(ply).unwrap();
        let played = review.game.history.get(ply).map(|e| e.mv);
        let recommended = board
            .legal_moves()
            .into_iter()
            .find(|m| Some(*m) != played)
            .unwrap();
        review.bests[ply] = Some(recommended);
        review.details[ply] = Some(AnalysisDetails {
            line: vec![recommended.to_string()],
            elapsed_ms: 1,
            budget_ms: ANALYSIS_LIMITS.1,
            requested_depth: ANALYSIS_LIMITS.0,
        });
    }
    review
}

#[test]
fn selected_move_and_recommendation_share_the_position_and_side() {
    let mut review = review();
    // Reverse navigation and repeated visits must use the same step as forward navigation.
    for ply in (1..=review.game.history.len()).chain((1..=review.game.history.len()).rev()) {
        review.ply = ply;
        let step = review.current_step();
        let entry = step.played.unwrap();
        assert_eq!(step.analysis_ply, ply - 1);
        assert_eq!(step.before, review.game.at_ply(ply - 1).unwrap());
        assert_eq!(step.before.turn, entry.mover);
        assert!(step.before.is_legal(entry.mv));
        let recommended = step.recommended.unwrap();
        assert_ne!(recommended, entry.mv);
        assert!(step.before.is_legal(recommended));
        assert_eq!(step.before.at(recommended.from).unwrap().side, entry.mover);
        assert_eq!(step.details.unwrap().line[0], recommended.to_string());
        // Playing the actual alternative reproduces exactly this recorded move's result.
        let mut after = step.before.clone();
        after.play(entry.mv).unwrap();
        assert_eq!(after, entry.board);
        assert_eq!(step.board, &after);
        assert!(step.board.at(entry.mv.from).is_none());
        assert_eq!(step.board.at(entry.mv.to).unwrap().side, entry.mover);
        let branch = review.game.branch_at(review.ply).unwrap();
        assert_eq!(&branch.board, step.board);
    }
}

#[test]
fn initial_position_has_only_the_first_recommendation() {
    for game in [Game::default(), review().game] {
        let mut review = ReviewSession::new(game, false);
        let recommended = review.game.start.legal_moves()[0];
        review.bests[0] = Some(recommended);
        let step = review.current_step();
        assert_eq!(step.analysis_ply, 0);
        assert_eq!(step.before, &review.game.start);
        assert_eq!(step.board, &review.game.start);
        assert!(step.played.is_none());
        assert_eq!(step.recommended, Some(recommended));
        assert!(step.details.is_none());
    }
}

#[test]
fn last_capture_and_finished_game_still_compare_the_last_recorded_move() {
    let mut review = review();
    review.ply = review.game.history.len();
    review.game.resign(Side::Black);
    for user_red in [false, true] {
        review.user_red = user_red;
        let step = review.current_step();
        let entry = step.played.unwrap();
        assert!(entry.capture);
        assert!(step.board.at(entry.mv.to).is_some());
        assert_eq!(step.board.at(entry.mv.to).unwrap().side, Side::Red);
        assert!(step.board.at(entry.mv.from).is_none());
        assert_eq!(step.before.turn, Side::Red);
        assert_eq!(step.before.at(entry.mv.to).unwrap().side, Side::Black);
        assert_ne!(step.recommended, review.bests[review.ply]);
        assert_eq!(step.recommended, review.bests[review.ply - 1]);
    }
}
