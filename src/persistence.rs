//! Independent data and cache workers; cache IO cannot queue ahead of game saves.
use crate::{
    game::Record,
    review::{CachedPosition, CachedReview},
    storage::{Data, Settings, SettingsChange, Storage},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Read,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
#[derive(Clone)]
pub enum Command {
    Save(Box<Record>, Option<Settings>),
    Settings(Settings),
    SettingsPatch(SettingsChange),
    Favorite(String, String),
    SetFavorite {
        id: String,
        favorite: bool,
        revision: u64,
    },
    Remove(String),
    RemoveChecked(Vec<(String, String)>),
    Repair,
    Export,
    Backups,
    ImportBackup(std::path::PathBuf),
    Clear,
    ClearDrafts,
    Activate(String),
    Rename(String, String, u64),
    Load,
    RestoreBackup,
    CacheLoad {
        key: String,
    },
    CachePut {
        key: String,
        entries: Vec<Option<CachedPosition>>,
    },
    CacheUpdate {
        key: String,
        len: usize,
        ply: usize,
        entry: CachedPosition,
    },
    Flush,
    Shutdown,
}
impl Command {
    pub fn save(record: Record, settings: Option<Settings>) -> Self {
        Self::Save(Box::new(record), settings)
    }
    pub fn changes_data(&self) -> bool {
        matches!(
            self,
            Self::Save(..)
                | Self::Settings(..)
                | Self::SettingsPatch(..)
                | Self::Favorite(..)
                | Self::SetFavorite { .. }
                | Self::Remove(..)
                | Self::RemoveChecked(..)
                | Self::Repair
                | Self::Export
                | Self::ImportBackup(..)
                | Self::Clear
                | Self::ClearDrafts
                | Self::Activate(..)
                | Self::Rename(..)
                | Self::Load
                | Self::RestoreBackup
        )
    }
    fn cache_command(&self) -> bool {
        matches!(
            self,
            Self::CacheLoad { .. }
                | Self::CachePut { .. }
                | Self::CacheUpdate { .. }
                | Self::Flush
                | Self::Shutdown
        )
    }
}
pub enum Response {
    Backups(Vec<std::path::PathBuf>),
    Exported(std::path::PathBuf),
    Data(Box<Data>),
    Settings(Settings),
    Cache {
        key: String,
        review: Option<CachedReview>,
    },
    /// Accepted in memory; CachePersisted and Flushed acknowledge durability.
    SavedCache,
    CachePersisted,
    CacheWarning(String),
    Flushed,
    Superseded,
}
pub struct Event {
    pub seq: u64,
    pub result: Result<Response, String>,
    pub retry: Option<Command>,
}
#[derive(Default, Serialize, Deserialize)]
struct CacheFile {
    fingerprint: String,
    reviews: BTreeMap<String, CachedReview>,
}
pub struct Worker {
    tx: mpsc::Sender<(u64, Command, Instant)>,
    pub rx: mpsc::Receiver<Event>,
    sequence: AtomicU64,
    handle: Option<JoinHandle<()>>,
}
impl Worker {
    /// Atomic replacement protects the library even if the process exits during IO.
    pub fn detach_on_exit(&mut self) {
        self.handle.take();
    }
    pub fn new(store: Storage) -> Self {
        let (tx, requests) = mpsc::channel::<(u64, Command, Instant)>();
        let (events, rx) = mpsc::channel();
        let (cache_tx, cache_rx) = mpsc::channel();
        let cache_events = events.clone();
        let path = store.path.with_file_name("analysis.json");
        let cache_handle = std::thread::spawn(move || cache_worker(path, cache_rx, cache_events));
        let handle = std::thread::spawn(move || {
            let mut carry = None;
            while let Some((mut seq, mut command, queued_at)) =
                carry.take().or_else(|| requests.recv().ok())
            {
                while matches!(
                    command,
                    Command::Save(..) | Command::Settings(..) | Command::SettingsPatch(..)
                ) {
                    let Ok((next_seq, mut next, next_queued)) = requests.try_recv() else {
                        break;
                    };
                    let replace = match (&command, &next) {
                        (Command::Settings(_), Command::Settings(_)) => true,
                        (Command::SettingsPatch(old), Command::SettingsPatch(new)) => {
                            let mut combined = old.clone();
                            combined.merge(new);
                            next = Command::SettingsPatch(combined);
                            true
                        }
                        (Command::Save(old, old_settings), Command::Save(new, new_settings)) => {
                            new.acknowledges(old)
                                && (old_settings.is_none() || new_settings.is_some())
                        }
                        _ => false,
                    };
                    if !replace {
                        carry = Some((next_seq, next, next_queued));
                        break;
                    }
                    let _ = events.send(Event {
                        seq,
                        result: Ok(Response::Superseded),
                        retry: None,
                    });
                    seq = next_seq;
                    command = next;
                }
                crate::diagnostics::measure(
                    "storage_queue_ms",
                    queued_at.elapsed().as_secs_f64() * 1000.,
                );
                if command.cache_command() {
                    let shutdown = matches!(command, Command::Shutdown);
                    if cache_tx.send((seq, command)).is_err() {
                        let _ = events.send(Event {
                            seq,
                            result: Ok(Response::CacheWarning("缓存线程已退出".into())),
                            retry: None,
                        });
                        let _ = events.send(Event {
                            seq,
                            result: Ok(Response::Flushed),
                            retry: None,
                        });
                    }
                    if shutdown {
                        break;
                    }
                    continue;
                }
                let start = Instant::now();
                let mut failure_kind = crate::diagnostics::FailureKind::Storage;
                let result = (|| -> Result<Response, String> {
                    let data = match &command {
                        Command::Save(r, s) => {
                            store.save_checked((**r).clone(), s.clone()).map_err(|e| {
                                failure_kind = e.kind;
                                e.message
                            })?
                        }
                        Command::Settings(s) => {
                            return store.save_settings(s.clone()).map(Response::Settings);
                        }
                        Command::SettingsPatch(change) => {
                            return store.patch_settings(change).map(Response::Settings);
                        }
                        Command::Favorite(id, op) => store.favorite_once(id, op)?,
                        Command::SetFavorite {
                            id,
                            favorite,
                            revision,
                        } => store.set_favorite(id, *favorite, *revision)?,
                        Command::Remove(id) => store.remove_by_id(id)?,
                        Command::RemoveChecked(targets) => store.remove_checked(targets)?,
                        Command::Repair => store.repair()?,
                        Command::Backups => return store.backups().map(Response::Backups),
                        Command::ImportBackup(path) => store.import_backup(path)?,
                        Command::Export => {
                            return store.export_backup().map(Response::Exported);
                        }
                        Command::Clear => store.update(|d| {
                            d.clear_archives();
                            Ok(())
                        })?,
                        Command::ClearDrafts => store.update(|d| {
                            d.clear_drafts();
                            Ok(())
                        })?,
                        Command::Rename(id, title, revision) => store.update(|d| {
                            let r = d
                                .active
                                .iter_mut()
                                .chain(d.drafts.iter_mut())
                                .chain(d.archives.iter_mut())
                                .find(|r| r.id == *id)
                                .ok_or("对局或棋谱已不存在")?;
                            let title: String = title
                                .trim()
                                .chars()
                                .filter(|c| !c.is_control())
                                .take(40)
                                .collect();
                            if r.title_revision != *revision {
                                if r.title == title
                                    && r.title_revision == revision.saturating_add(1)
                                {
                                    return Ok(());
                                }
                                return Err(
                                    "[conflict] 名称已由另一操作更新，请重新读取后命名".into()
                                );
                            }
                            r.title = title;
                            r.title_revision = revision.checked_add(1).ok_or("名称修订号已满")?;
                            Ok(())
                        })?,
                        Command::Activate(id) => store.activate(id)?,
                        Command::Load => store.load()?,
                        Command::RestoreBackup => store.restore_backup()?,
                        _ => unreachable!(),
                    };
                    Ok(Response::Data(Box::new(data)))
                })();
                crate::diagnostics::measure("storage_ms", start.elapsed().as_secs_f64() * 1000.);
                if let Err(e) = &result {
                    crate::diagnostics::failure(failure_kind, "persistence", e);
                }
                let retry = if result.is_err() && command.changes_data() {
                    Some(command)
                } else {
                    None
                };
                if events.send(Event { seq, result, retry }).is_err() {
                    break;
                }
            }
            drop(cache_tx);
            let _ = cache_handle.join();
        });
        Self {
            tx,
            rx,
            sequence: AtomicU64::new(0),
            handle: Some(handle),
        }
    }
    pub fn send(&self, command: Command) -> Result<u64, String> {
        let seq = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        self.tx
            .send((seq, command, Instant::now()))
            .map_err(|_| "存档线程已退出".to_string())?;
        Ok(seq)
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.send(Command::Shutdown);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
fn read_cache(path: &std::path::Path, fingerprint: &str) -> CacheFile {
    std::fs::metadata(path)
        .ok()
        .filter(|m| m.len() <= 16 * 1024 * 1024)
        .and_then(|_| std::fs::read(path).ok())
        .and_then(|b| serde_json::from_slice::<CacheFile>(&b).ok())
        .filter(|c| {
            !fingerprint.is_empty() && c.fingerprint == fingerprint && c.reviews.len() <= 100
        })
        .unwrap_or_else(|| CacheFile {
            fingerprint: fingerprint.into(),
            ..Default::default()
        })
}
fn merge_review(old: &mut CachedReview, new: &CachedReview) {
    if old.entries.len() != new.entries.len() {
        *old = new.clone();
        return;
    }
    for (a, b) in old.entries.iter_mut().zip(&new.entries) {
        if let Some(b) = b
            && a.as_ref().is_none_or(|a| b.supersedes(a))
        {
            *a = Some(b.clone());
        }
    }
    old.updated_at = old.updated_at.max(new.updated_at);
}
fn write_cache(path: &std::path::Path, cache: &mut CacheFile) -> Result<(), String> {
    let parent = path.parent().ok_or("缓存目录无效")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let _lock = Storage::new(path.into()).lock()?;
    let mut merged = read_cache(path, &cache.fingerprint);
    for (key, value) in &cache.reviews {
        if let Some(old) = merged.reviews.get_mut(key) {
            merge_review(old, value);
        } else {
            merged.reviews.insert(key.clone(), value.clone());
        }
    }
    let mut bytes = serde_json::to_vec(&merged).map_err(|e| e.to_string())?;
    while merged.reviews.len() > 100 || bytes.len() > 16 * 1024 * 1024 {
        let key = merged
            .reviews
            .iter()
            .min_by_key(|(_, r)| r.updated_at)
            .map(|(k, _)| k.clone())
            .ok_or("分析缓存过大")?;
        merged.reviews.remove(&key);
        bytes = serde_json::to_vec(&merged).map_err(|e| e.to_string())?;
    }
    Storage::atomic_write(path, &bytes)?;
    *cache = merged;
    Ok(())
}
fn cache_worker(
    path: std::path::PathBuf,
    rx: mpsc::Receiver<(u64, Command)>,
    tx: mpsc::Sender<Event>,
) {
    let mut cache = CacheFile::default();
    let mut initialized = false;
    let mut dirty = false;
    let mut deadline = Instant::now();
    let emit = |seq, result| {
        let _ = tx.send(Event {
            seq,
            result: Ok(result),
            retry: None,
        });
    };
    loop {
        if dirty && Instant::now() >= deadline {
            match write_cache(&path, &mut cache) {
                Ok(()) => {
                    dirty = false;
                    emit(0, Response::CachePersisted);
                }
                Err(e) => emit(0, Response::CacheWarning(e)),
            }
            deadline = Instant::now() + Duration::from_secs(2);
        }
        let message = if dirty {
            rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        } else {
            rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        let (seq, command) = match message {
            Ok(m) => m,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                match write_cache(&path, &mut cache) {
                    Ok(()) => {
                        dirty = false;
                        emit(0, Response::CachePersisted);
                    }
                    Err(e) => emit(0, Response::CacheWarning(e)),
                }
                deadline = Instant::now() + Duration::from_secs(2);
                continue;
            }
            Err(_) => {
                if dirty {
                    let _ = write_cache(&path, &mut cache);
                }
                return;
            }
        };
        if matches!(command, Command::Shutdown | Command::Flush) {
            if dirty {
                match write_cache(&path, &mut cache) {
                    Ok(()) => dirty = false,
                    Err(e) => emit(seq, Response::CacheWarning(e)),
                }
            }
            if matches!(command, Command::Shutdown) {
                return;
            }
            emit(seq, Response::Flushed);
            continue;
        }
        if !initialized {
            match asset_fingerprint() {
                Ok(f) => cache = read_cache(&path, &f),
                Err(e) => emit(seq, Response::CacheWarning(e)),
            }
            initialized = !cache.fingerprint.is_empty();
        }
        match command {
            Command::CacheUpdate {
                key,
                len,
                ply,
                entry,
            } => {
                if len <= crate::game::MAX_PLIES + 1 && ply < len && !cache.fingerprint.is_empty() {
                    let review = cache.reviews.entry(key).or_insert_with(|| CachedReview {
                        entries: vec![None; len],
                        updated_at: 0,
                    });
                    if review.entries.len() != len {
                        review.entries = vec![None; len];
                    }
                    {
                        if review.entries[ply]
                            .as_ref()
                            .is_none_or(|old| entry.supersedes(old))
                        {
                            review.entries[ply] = Some(entry);
                        }
                        review.updated_at = crate::game::now_ms();
                        if !dirty {
                            deadline = Instant::now() + Duration::from_secs(2);
                        }
                        dirty = true;
                    }
                }
                emit(seq, Response::SavedCache);
            }
            Command::CacheLoad { key } => {
                if !dirty && !cache.fingerprint.is_empty() {
                    cache = read_cache(&path, &cache.fingerprint);
                }
                let review = cache.reviews.get(&key).cloned();
                emit(seq, Response::Cache { key, review });
            }
            Command::CachePut { key, entries } => {
                if entries.len() <= crate::game::MAX_PLIES + 1 && !cache.fingerprint.is_empty() {
                    let value = CachedReview {
                        entries,
                        updated_at: crate::game::now_ms(),
                    };
                    if let Some(old) = cache.reviews.get_mut(&key) {
                        merge_review(old, &value);
                    } else {
                        cache.reviews.insert(key, value);
                    }
                    if !dirty {
                        deadline = Instant::now() + Duration::from_secs(2);
                    }
                    dirty = true;
                }
                emit(seq, Response::SavedCache);
            }
            _ => unreachable!(),
        }
    }
}
pub fn asset_fingerprint() -> Result<String, String> {
    static FINGERPRINT: std::sync::OnceLock<std::sync::Mutex<Option<(String, String)>>> =
        std::sync::OnceLock::new();
    let nnue = crate::platform::nnue_path()?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let paths = vec![nnue, crate::platform::engine_path()?];
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let paths = vec![nnue];
    let stamp = format!(
        "{:?}",
        paths
            .iter()
            .map(|p| std::fs::metadata(p).map(|m| (p, m.len(), m.modified().ok())))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    );
    let mut saved = FINGERPRINT
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .map_err(|e| e.to_string())?;
    if let Some((s, f)) = &*saved
        && *s == stamp
    {
        return Ok(f.clone());
    }
    let start = Instant::now();
    let mut hash = Sha256::new();
    hash.update(include_bytes!("../tool/engine_manifest.json"));
    hash.update(b"rules-4;score-3;threads-1;hash-32");
    hash.update(env!("CARGO_PKG_VERSION"));
    for p in paths {
        let mut f = std::fs::File::open(p).map_err(|e| e.to_string())?;
        let mut buf = [0u8; 65536];
        loop {
            let n = f.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
        }
    }
    let fingerprint = format!("{:x}", hash.finalize());
    *saved = Some((stamp, fingerprint.clone()));
    crate::diagnostics::measure("asset_hash_ms", start.elapsed().as_secs_f64() * 1000.);
    Ok(fingerprint)
}
