//! Bounded deterministic mutation fuzzing, usable on stable Rust and scheduled CI.
use tst_xiangqi::{
    game::{Game, Record},
    review::CachedReview,
    rules::Board,
    storage::Storage,
};
fn main() {
    let cases = std::env::args()
        .nth(1)
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(10000)
        .min(1_000_000);
    let mut seed = std::env::args()
        .nth(2)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(936137555);
    let initial_seed = seed;
    let record = Game::default().snapshot(true, 0, false);
    let corpus=[Board::default().fen().into_bytes(),serde_json::to_vec(&record).unwrap(),br#"{"entries":[{"score":{"kind":"cp","value":0},"depth":12,"best":"h2e2"}],"updated_at":0}"#.to_vec(),br#"{"version":999,"settings":{},"active":null,"archives":[],"drafts":[]}"#.to_vec()];
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::new(dir.path().join("state.json"));
    for i in 0..cases {
        let mut bytes = corpus[i % corpus.len()].clone();
        for _ in 0..1 + i % 8 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let at = seed as usize % (bytes.len() + 1);
            match (seed >> 32) % 4 {
                0 => {
                    bytes.insert(at, (seed >> 16) as u8);
                }
                1 if at < bytes.len() => {
                    bytes.remove(at);
                }
                2 if at < bytes.len() => bytes[at] = (seed >> 8) as u8,
                _ => {
                    bytes.extend_from_slice(b"\xff\x00\xf0\x9f\x98\x80");
                }
            }
        }
        let _ = String::from_utf8_lossy(&bytes).parse::<Board>();
        if let Ok(record) = serde_json::from_slice::<Record>(&bytes) {
            let _ = record.replay();
        }
        if let Ok(cache) = serde_json::from_slice::<CachedReview>(&bytes) {
            for (ply, entry) in cache.entries.iter().take(4097).enumerate() {
                if let Some(entry) = entry {
                    let _ = entry.valid_for(&Game::default(), ply);
                }
            }
        }
        if i % 16 == 0 {
            std::fs::write(&store.path, &bytes).unwrap();
            if store.load().is_err() {
                assert!(store.update(|_| Ok(())).is_err());
                assert_eq!(std::fs::read(&store.path).unwrap(), bytes);
            }
        }
    }
    println!(
        "{}",
        serde_json::json!({"cases":cases,"seed":initial_seed,"maxInputBytes":4096,"result":"no panic; rejected library bytes preserved","scope":"bounded deterministic mutation corpus, not exhaustive"})
    );
}
