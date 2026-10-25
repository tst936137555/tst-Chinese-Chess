//! Bounded serial record loading. The UI applies only the reply it still awaits.
use crate::game::{Game, Record};
use std::sync::mpsc;

pub struct Reply {
    pub id: u64,
    pub record: Record,
    pub review: bool,
    pub result: Result<Game, String>,
}
pub struct Loader {
    tx: mpsc::SyncSender<(u64, Record, bool)>,
    pub rx: mpsc::Receiver<Reply>,
    sequence: u64,
}
impl Default for Loader {
    fn default() -> Self {
        let (tx, requests) = mpsc::sync_channel::<(u64, Record, bool)>(1);
        let (replies, rx) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok((id, record, review)) = requests.recv() {
                let result = record.replay();
                if replies
                    .send(Reply {
                        id,
                        record,
                        review,
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
            .try_send((id, record, review))
            .map_err(|_| "棋谱读取繁忙，请稍后重试")?;
        self.sequence = id;
        Ok(id)
    }
}
