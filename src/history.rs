//! Validate only the changed suffix, including when a caller undoes or forks a history.
use crate::rules::{Board, Move};
#[derive(Default)]
pub struct HistoryVerifier {
    identity: String,
    start: Option<Board>,
    moves: Vec<Move>,
    boards: Vec<Board>,
}
impl HistoryVerifier {
    pub fn verify(
        &mut self,
        identity: &str,
        start: &Board,
        moves: &[Move],
        expected: &Board,
    ) -> Result<usize, String> {
        if moves.len() > crate::game::MAX_PLIES {
            return Err("引擎历史过长".into());
        }
        if self.identity != identity || self.start.as_ref() != Some(start) {
            self.identity = identity.into();
            self.start = Some(start.clone());
            self.moves.clear();
            self.boards.clear();
        }
        let common = self
            .moves
            .iter()
            .zip(moves)
            .take_while(|(a, b)| a == b)
            .count();
        self.moves.truncate(common);
        self.boards.truncate(common);
        let mut board = self.boards.last().unwrap_or(start).clone();
        for &mv in &moves[common..] {
            board.play(mv)?;
            self.moves.push(mv);
            self.boards.push(board.clone());
        }
        if board != *expected {
            return Err("引擎历史与当前局面不一致".into());
        }
        Ok(moves.len() - common)
    }
}
