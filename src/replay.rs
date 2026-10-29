//! Bounded serial record loading. The UI applies only the reply it still awaits.
use crate::game::{Game, Record};
use std::sync::mpsc;

pub struct Reply {
    pub id: u64,
    pub record: Record,
    pub review: bool,
    pub branch: bool,
    pub result: Result<Game, String>,
}
pub struct Loader {
    tx: mpsc::SyncSender<(u64, Record, bool, Option<usize>)>,
    pub rx: mpsc::Receiver<Reply>,
    sequence: u64,
}
impl Default for Loader {
    fn default() -> Self {
        let (tx, requests) = mpsc::sync_channel::<(u64, Record, bool, Option<usize>)>(1);
        let (replies, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            while let Ok((id, record, review, branch)) = requests.recv() {
                let result = record.replay().and_then(|game| match branch {
                    Some(ply) => game.branch_at(ply),
                    None => Ok(game),
                });
                if replies
                    .send(Reply {
                        id,
                        record,
                        review,
                        branch: branch.is_some(),
                        result,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            tx,
            rx,
            sequence: 0,
        }
    }
}
impl Loader {
    pub fn request(&mut self, record: Record, review: bool) -> Result<u64, String> {
        let id = self.sequence.checked_add(1).ok_or("读取请求已满")?;
        self.tx
            .try_send((id, record, review, None))
            .map_err(|_| "棋谱读取繁忙，请稍后重试")?;
        self.sequence = id;
        Ok(id)
    }
    pub fn branch(&mut self, record: Record, ply: usize) -> Result<u64, String> {
        let id = self.sequence.checked_add(1).ok_or("读取请求已满")?;
        self.tx
            .try_send((id, record, false, Some(ply)))
            .map_err(|_| "棋谱读取繁忙，请稍后重试")?;
        self.sequence = id;
        Ok(id)
    }
}
