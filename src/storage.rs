//! Versioned, atomic storage. UI writes run on a serial worker; all transactions lock on disk.
use crate::game::{Outcome, Record};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub const SCHEMA_VERSION: u32 = 7;
const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_IMPORT_RECORDS: usize = 1000;
pub const MAX_DRAFTS: usize = 50;
pub const MAX_ARCHIVES: usize = 100;
pub const MAX_FAVORITES: usize = 50;
const MAX_LEGACY_OPERATIONS: usize = 4096;
const MAX_RETIRED_IDS: usize = 4096;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub user_red: bool,
    pub level: usize,
    pub sound: bool,
    pub music_enabled: bool,
    pub effects_enabled: bool,
    pub music_volume: f32,
    pub effects_volume: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            user_red: true,
            level: 0,
            sound: true,
            music_enabled: true,
            effects_enabled: true,
            music_volume: 0.28,
            effects_volume: 0.72,
        }
    }
}
impl Settings {
    fn sanitize(&mut self) {
        self.level = self.level.min(4);
        self.music_volume = self.music_volume.clamp(0., 1.);
        self.effects_volume = self.effects_volume.clamp(0., 1.);
    }
}
/// Only fields edited by this window are applied to the latest preferences on disk.
#[derive(Clone, Debug)]
pub struct SettingsChange {
    pub before: Settings,
    pub after: Settings,
}
impl SettingsChange {
    /// Compose consecutive edits while preserving the oldest baseline per field.
    pub fn merge(&mut self, newer: &Self) {
        macro_rules! merge { ($($field:ident),+) => { $(
            if newer.before.$field != newer.after.$field {
                if self.before.$field == self.after.$field {
                    self.before.$field = newer.before.$field;
                }
                self.after.$field = newer.after.$field;
                // Returning to the initial value is still an explicit edit.
                if self.before.$field == self.after.$field {
                    self.before.$field = newer.before.$field;
                }
            }
        )+ }; }
        merge!(
            user_red,
            level,
            sound,
            music_enabled,
            effects_enabled,
            music_volume,
            effects_volume
        );
    }
    pub fn supersede(&mut self, newer: &Self) {
        macro_rules! replaced { ($($field:ident),+) => { $(if newer.before.$field != newer.after.$field {
            self.before.$field = self.after.$field;
        })+ }; }
        replaced!(
            user_red,
            level,
            sound,
            music_enabled,
            effects_enabled,
            music_volume,
            effects_volume
        );
    }
    pub fn apply(&self, settings: &mut Settings) {
        macro_rules! changed {
            ($($field:ident),+) => { $(if self.before.$field != self.after.$field {
                settings.$field = self.after.$field;
            })+ };
        }
        changed!(
            user_red,
            level,
            sound,
            music_enabled,
            effects_enabled,
            music_volume,
            effects_volume
        );
        settings.sanitize();
    }
    pub fn is_empty(&self) -> bool {
        self.before == self.after
    }
    pub fn acknowledged(&self, settings: &Settings) -> bool {
        let mut expected = settings.clone();
        self.apply(&mut expected);
        expected == *settings
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Data {
    pub version: u32,
    pub settings: Settings,
    pub active: Option<Record>,
    pub archives: Vec<Record>,
    #[serde(default)]
    pub drafts: Vec<Record>,
    #[serde(default)]
    pub applied_operations: Vec<String>,
    #[serde(default)]
    pub deleted_ids: BTreeSet<String>,
    /// Compacted identities remain barred from automatic stale-window writes.
    #[serde(default)]
    pub retired_before_nanos: u64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
impl Default for Data {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            settings: Settings::default(),
            active: None,
            archives: Vec::new(),
            drafts: Vec::new(),
            applied_operations: Vec::new(),
            deleted_ids: BTreeSet::new(),
            retired_before_nanos: 0,
            extra: BTreeMap::new(),
        }
    }
}
impl Data {
    fn compact_retired(&mut self) {
        if self.deleted_ids.len() > MAX_RETIRED_IDS {
            self.retired_before_nanos = self
                .retired_before_nanos
                .max(crate::game::identity_time(&crate::game::new_id()).unwrap_or(0));
            while self.deleted_ids.len() > MAX_RETIRED_IDS {
                self.deleted_ids.pop_first();
            }
        }
    }
    pub fn clear_archives(&mut self) {
        self.deleted_ids
            .extend(self.archives.drain(..).map(|r| r.id));
    }
    pub fn clear_drafts(&mut self) {
        self.deleted_ids.extend(self.drafts.drain(..).map(|r| r.id));
    }
    fn trim_archives(&mut self) {
        let (mut plain, mut fav) = (0, 0);
        self.archives.retain(|r| {
            let n = if r.favorite { &mut fav } else { &mut plain };
            *n += 1;
            let keep = *n
                <= if r.favorite {
                    MAX_FAVORITES
                } else {
                    MAX_ARCHIVES
                };
            if !keep {
                self.deleted_ids.insert(r.id.clone());
            }
            keep
        });
    }
    fn validate(&self) -> Result<(), String> {
        if self.retired_before_nanos > crate::game::MAX_REVISION {
            return Err("存档身份屏障超出有效范围，原文件已保留".into());
        }
        if self
            .deleted_ids
            .iter()
            .any(|id| id.is_empty() || id.len() > 128)
        {
            return Err("删除记录身份无效".into());
        }
        if self.archives.len() > MAX_IMPORT_RECORDS || self.drafts.len() > MAX_DRAFTS {
            return Err("存档局数超限，原文件已保留".into());
        }
        let mut identities = BTreeSet::new();
        for record in self.active.iter().chain(&self.archives).chain(&self.drafts) {
            validate_record(record)?;
            if record.id.is_empty() || !identities.insert(&record.id) {
                return Err("存档存在重复或空的对局身份，原文件已保留".into());
            }
            if self.deleted_ids.contains(&record.id) {
                return Err("存档包含已删除的对局身份，原文件已保留".into());
            }
        }
        if self
            .active
            .iter()
            .chain(&self.drafts)
            .any(|r| r.result != Outcome::Playing)
            || self.archives.iter().any(|r| r.result == Outcome::Playing)
        {
            return Err("存档中的活动对局与归档状态不一致，原文件已保留".into());
        }
        Ok(())
    }
}
/// Validate metadata on every read; cache only the complete rules/replay input.
/// Names, favorites and identities do not change the legality of a history.
fn validate_record(record: &Record) -> Result<(), String> {
    use std::{
        collections::VecDeque,
        sync::{Mutex, OnceLock},
    };
    static VERIFIED: OnceLock<Mutex<VecDeque<[u8; 32]>>> = OnceLock::new();
    record.validate_metadata()?;
    let key = record.semantic_key();
    let cache = VERIFIED.get_or_init(Default::default);
    if cache.lock().is_ok_and(|entries| entries.contains(&key)) {
        return Ok(());
    }
    record.replay()?;
    if let Ok(mut entries) = cache.lock() {
        if entries.len() >= 512 {
            entries.pop_front();
        }
        entries.push_back(key);
    }
    Ok(())
}
#[derive(Clone)]
pub struct Storage {
    pub path: PathBuf,
}
#[derive(Debug)]
pub struct StorageFailure {
    pub kind: crate::diagnostics::FailureKind,
    pub message: String,
}
#[derive(Serialize, Deserialize)]
struct SettingsFile {
    version: u32,
    settings: Settings,
}
impl Storage {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    fn read(path: &Path) -> Result<Vec<u8>, String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_FILE_BYTES {
            return Err("存档超过 32 MiB 限制，原文件已保留".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err("存档超过 32 MiB 限制".into());
        }
        Ok(bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Data, String> {
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| format!("存档损坏，原文件已保留：{e}"))?;
        if value
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|n| n > SCHEMA_VERSION as u64)
        {
            return Err("存档来自更新版本，已禁止写入，请升级程序".into());
        }
        let mut d: Data = serde_json::from_value(value)
            .map_err(|e| format!("存档结构无效，原文件已保留：{e}"))?;
        for r in d
            .active
            .iter_mut()
            .chain(d.archives.iter_mut())
            .chain(d.drafts.iter_mut())
        {
            r.normalize();
        }
        d.validate()?;
        crate::game::advance_identity_clock(d.retired_before_nanos);
        d.settings.sanitize();
        d.version = SCHEMA_VERSION;
        Ok(d)
    }
    fn load_state(&self) -> Result<Data, String> {
        match fs::metadata(&self.path) {
            Ok(_) => {
                let started = Instant::now();
                let result = Self::decode(&Self::read(&self.path)?);
                crate::diagnostics::measure(
                    "storage_read_validate_ms",
                    started.elapsed().as_secs_f64() * 1000.,
                );
                result
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                match fs::metadata(self.path.with_extension("bak")) {
                    Ok(_) => Err("主存档缺失，但仍有备份；请先恢复备份，原备份已保护".into()),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Data::default()),
                    Err(e) => Err(format!("无法检查存档备份：{e}")),
                }
            }
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn settings_path(&self) -> PathBuf {
        self.path.with_extension("settings.json")
    }
    fn decode_settings(path: &Path) -> Result<Settings, String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        file.take(16_385)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 16_384 {
            return Err("设置文件过大".into());
        }
        let mut saved: SettingsFile = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if saved.version != 1 {
            return Err("设置来自不支持的版本，请升级程序".into());
        }
        saved.settings.sanitize();
        Ok(saved.settings)
    }
    pub fn load(&self) -> Result<Data, String> {
        let mut data = self.load_state()?;
        data.settings = self.load_preferences(data.settings);
        Ok(data)
    }
    /// Read preferences even when the independent game library is damaged.
    pub fn load_preferences(&self, mut fallback: Settings) -> Settings {
        let path = self.settings_path();
        match Self::decode_settings(&path) {
            Ok(settings) => fallback = settings,
            Err(error) => {
                if path.exists() {
                    crate::diagnostics::record("settings-recovery", &error);
                }
                if let Ok(settings) = Self::decode_settings(&path.with_extension("bak")) {
                    fallback = settings;
                }
            }
        }
        fallback
    }
    fn write_settings(&self, settings: &Settings) -> Result<(), String> {
        let path = self.settings_path();
        // A missing primary must not permit a downgrade of its future-version backup.
        let guard = if path.exists() {
            path.clone()
        } else {
            path.with_extension("bak")
        };
        if let Ok(bytes) = Self::read(&guard)
            && serde_json::from_slice::<serde_json::Value>(&bytes)
                .ok()
                .and_then(|v| v.get("version").and_then(|v| v.as_u64()))
                .is_some_and(|version| version > 1)
        {
            return Err("设置来自更新版本，已禁止覆盖".into());
        }
        if path.exists() {
            let bytes = Self::read(&path)?;
            if serde_json::from_slice::<serde_json::Value>(&bytes)
                .ok()
                .and_then(|v| v.get("version").and_then(|v| v.as_u64()))
                .is_some_and(|version| version > 1)
            {
                return Err("设置来自更新版本，已禁止覆盖".into());
            }
            if Self::decode_settings(&path).is_ok_and(|old| old == *settings) {
                return Ok(());
            }
            let backup = if Self::decode_settings(&path).is_ok() {
                path.with_extension("bak")
            } else {
                path.with_extension(format!("damaged-{}.json", crate::game::new_id()))
            };
            Self::atomic_write(&backup, &bytes)?;
        }
        let bytes = serde_json::to_vec(&SettingsFile {
            version: 1,
            settings: settings.clone(),
        })
        .map_err(|e| e.to_string())?;
        Self::atomic_write(&path, &bytes)
    }
    /// Preferences have their own atomic file; changing volume never scans the game library.
    pub fn save_settings(&self, mut settings: Settings) -> Result<Settings, String> {
        fs::create_dir_all(self.path.parent().ok_or("设置目录无效")?).map_err(|e| e.to_string())?;
        let _lock = self.lock()?;
        settings.sanitize();
        self.write_settings(&settings)?;
        Ok(settings)
    }
    pub fn patch_settings(&self, change: &SettingsChange) -> Result<Settings, String> {
        fs::create_dir_all(self.path.parent().ok_or("设置目录无效")?).map_err(|e| e.to_string())?;
        let _lock = self.lock()?;
        let path = self.settings_path();
        let mut settings = Self::decode_settings(&path)
            .or_else(|_| Self::decode_settings(&path.with_extension("bak")))
            .unwrap_or_else(|_| change.before.clone());
        change.apply(&mut settings);
        // write_settings also preserves damaged files and rejects future schemas.
        self.write_settings(&settings)?;
        Ok(settings)
    }
    pub(crate) fn lock(&self) -> Result<File, String> {
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.path.with_extension("lock"))
            .map_err(|e| e.to_string())?;
        let start = Instant::now();
        loop {
            match lock.try_lock() {
                Ok(()) => {
                    crate::diagnostics::measure(
                        "storage_lock_wait_ms",
                        start.elapsed().as_secs_f64() * 1000.,
                    );
                    return Ok(lock);
                }
                Err(std::fs::TryLockError::WouldBlock)
                    if start.elapsed() < Duration::from_secs(2) =>
                {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(e) => return Err(format!("存档被占用，请重试：{e}")),
            }
        }
    }
    pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
        let parent = path.parent().ok_or("存档目录无效")?;
        let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        let written = Instant::now();
        tmp.write_all(bytes).map_err(|e| e.to_string())?;
        crate::diagnostics::measure("storage_write_ms", written.elapsed().as_secs_f64() * 1000.);
        let synced = Instant::now();
        tmp.as_file().sync_all().map_err(|e| e.to_string())?;
        crate::diagnostics::measure("storage_sync_ms", synced.elapsed().as_secs_f64() * 1000.);
        let replaced = Instant::now();
        tmp.persist(path).map_err(|e| e.to_string())?;
        crate::diagnostics::measure(
            "storage_replace_ms",
            replaced.elapsed().as_secs_f64() * 1000.,
        );
        #[cfg(unix)]
        File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn update(&self, op: impl FnOnce(&mut Data) -> Result<(), String>) -> Result<Data, String> {
        let parent = self.path.parent().ok_or("存档目录无效")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let _lock = self.lock()?;
        let mut data = self.load()?;
        let old_settings = data.settings.clone();
        op(&mut data)?;
        data.compact_retired();
        data.version = SCHEMA_VERSION;
        data.settings.sanitize();
        data.validate()?;
        let serialized = Instant::now();
        let bytes = serde_json::to_vec(&data).map_err(|e| e.to_string())?;
        crate::diagnostics::measure(
            "storage_serialize_ms",
            serialized.elapsed().as_secs_f64() * 1000.,
        );
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err("存档超过 32 MiB 限制".into());
        }
        let previous = if self.path.is_file() {
            Some(Self::read(&self.path)?)
        } else {
            None
        };
        if previous.as_deref() != Some(bytes.as_slice()) {
            if let Some(previous) = previous {
                self.checkpoint(&previous)?;
                Self::atomic_write(&self.path.with_extension("bak"), &previous)?;
            }
            Self::atomic_write(&self.path, &bytes)?;
        } else if !self.path.with_extension("bak").is_file() {
            // Seed the first recovery point even when the second transaction is a no-op.
            Self::atomic_write(&self.path.with_extension("bak"), &bytes)?;
        }
        if data.settings != old_settings {
            self.write_settings(&data.settings)?;
        }
        Ok(data)
    }
    fn checkpoint(&self, bytes: &[u8]) -> Result<(), String> {
        let directory = self.path.parent().ok_or("存档目录无效")?.join("backups");
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let hour = crate::game::now_ms() / 3_600_000;
        let path = directory.join(format!("checkpoint-{hour:016}.json"));
        if !path.exists() {
            Self::atomic_write(&path, bytes)?;
        }
        let mut entries: Vec<_> = fs::read_dir(&directory)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("checkpoint-") && n.ends_with(".json"))
            })
            .collect();
        entries.sort();
        let remove = entries.len().saturating_sub(8);
        for entry in entries.into_iter().take(remove) {
            fs::remove_file(entry).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn export_backup(&self) -> Result<PathBuf, String> {
        let _lock = self.lock()?;
        let data = self.load()?;
        let directory = self.path.parent().ok_or("存档目录无效")?.join("backups");
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let path = directory.join(format!("export-{}.json", crate::game::new_id()));
        Self::atomic_write(
            &path,
            &serde_json::to_vec(&data).map_err(|e| e.to_string())?,
        )?;
        Ok(path)
    }
    pub fn backups(&self) -> Result<Vec<PathBuf>, String> {
        let directory = self.path.parent().ok_or("存档目录无效")?.join("backups");
        if !directory.exists() {
            return Ok(Vec::new());
        }
        let mut paths: Vec<_> = fs::read_dir(directory)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        paths.sort_by_key(|p| std::cmp::Reverse(fs::metadata(p).and_then(|m| m.modified()).ok()));
        paths.truncate(20);
        Ok(paths)
    }
    pub fn import_backup(&self, path: &Path) -> Result<Data, String> {
        let data = Self::decode(&Self::read(path)?)?;
        let mut records: Vec<_> = data
            .active
            .into_iter()
            .chain(data.drafts)
            .chain(data.archives)
            .collect();
        let current = self.load()?;
        for record in &mut records {
            if current.deleted_ids.contains(&record.id) {
                let mut identity = format!("restored-{}", record.content_key());
                while current.deleted_ids.contains(&identity) {
                    identity = format!("restored-{:x}", sha2::Sha256::digest(identity.as_bytes()));
                }
                record.fork_identity();
                record.id = identity;
            }
        }
        let temp = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
        fs::write(
            temp.path(),
            serde_json::to_vec(&records).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        self.import_legacy(temp.path()).map(|(data, _)| data)
    }
    /// Explicit repair only; the original remains available even when individual records fail.
    pub fn repair(&self) -> Result<Data, String> {
        let _lock = self.lock()?;
        let bytes = Self::read(&self.path)?;
        let mut value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if value
            .get("version")
            .and_then(|v| v.as_u64())
            .is_none_or(|v| v > SCHEMA_VERSION as u64)
        {
            return Err("存档版本不支持修复，请升级程序".into());
        }
        let mut candidates = Vec::new();
        if let Some(active) = value.get_mut("active")
            && !active.is_null()
        {
            candidates.push(active.take());
        }
        for key in ["drafts", "archives"] {
            if let Some(records) = value.get_mut(key) {
                if let Some(array) = records.as_array_mut() {
                    candidates.append(array);
                } else {
                    return Err("棋谱列表结构损坏，原文件已保留".into());
                }
            }
        }
        let mut data = Self::decode(&serde_json::to_vec(&value).map_err(|e| e.to_string())?)?;
        let total = candidates.len();
        let mut seen = BTreeSet::new();
        for candidate in candidates {
            let Ok(mut record) = serde_json::from_value::<Record>(candidate) else {
                continue;
            };
            record.normalize();
            if validate_record(&record).is_err()
                || data.deleted_ids.contains(&record.id)
                || !seen.insert(record.id.clone())
            {
                continue;
            }
            if record.result == Outcome::Playing {
                if data.active.is_none() {
                    data.active = Some(record);
                } else if data.drafts.len() < MAX_DRAFTS {
                    data.drafts.push(record);
                }
            } else if data
                .archives
                .iter()
                .filter(|r| r.favorite == record.favorite)
                .count()
                < if record.favorite {
                    MAX_FAVORITES
                } else {
                    MAX_ARCHIVES
                }
            {
                data.archives.push(record);
            }
        }
        data.validate()?;
        let recovered = data.active.iter().count() + data.drafts.len() + data.archives.len();
        if recovered == 0 && total > 0 {
            return Err("没有可恢复的有效棋谱，原文件已保留".into());
        }
        let original = self
            .path
            .with_extension(format!("damaged-{}.json", crate::game::new_id()));
        Self::atomic_write(&original, &bytes)?;
        Self::atomic_write(
            &original.with_extension("txt"),
            format!("共 {total} 局，恢复 {recovered} 局；原文件保留。\n").as_bytes(),
        )?;
        Self::atomic_write(
            &self.path,
            &serde_json::to_vec(&data).map_err(|e| e.to_string())?,
        )?;
        self.load()
    }
    pub fn remove_checked(&self, targets: &[(String, String)]) -> Result<Data, String> {
        self.update(|data| {
            for (id, token) in targets {
                if data.active.as_ref().is_some_and(|r| r.id == *id)
                    || data
                        .archives
                        .iter()
                        .chain(&data.drafts)
                        .any(|r| r.id == *id && r.version_token != *token)
                {
                    return Err("[conflict] 对局已更新，请重新读取后确认删除".into());
                }
            }
            let ids: BTreeSet<_> = targets.iter().map(|(id, _)| id.clone()).collect();
            data.deleted_ids.extend(
                data.archives
                    .iter()
                    .chain(&data.drafts)
                    .filter(|r| ids.contains(&r.id))
                    .map(|r| r.id.clone()),
            );
            data.archives.retain(|r| !ids.contains(&r.id));
            data.drafts.retain(|r| !ids.contains(&r.id));
            Ok(())
        })
    }
    /// Explicit recovery preserves the broken file and never silently replaces a future schema.
    pub fn restore_backup(&self) -> Result<Data, String> {
        let _lock = self.lock()?;
        match fs::metadata(&self.path) {
            Ok(_) => {
                let bytes = Self::read(&self.path)?;
                if serde_json::from_slice::<serde_json::Value>(&bytes)
                    .ok()
                    .and_then(|v| v.get("version").and_then(|n| n.as_u64()))
                    .is_some_and(|n| n > SCHEMA_VERSION as u64)
                {
                    return Err("不能用旧备份替换未来版本存档".into());
                }
                let mut damaged = self.path.with_extension("damaged.json");
                if damaged.exists() {
                    damaged = self
                        .path
                        .with_extension(format!("damaged-{}.json", crate::game::new_id()));
                }
                Self::atomic_write(&damaged, &bytes)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
        let bytes = Self::read(&self.path.with_extension("bak"))?;
        Self::decode(&bytes)?;
        Self::atomic_write(&self.path, &bytes)?;
        self.load()
    }
    pub fn save_game(&self, record: Record, settings: Option<Settings>) -> Result<Data, String> {
        self.save_checked(record, settings).map_err(|e| e.message)
    }
    pub fn save_checked(
        &self,
        mut record: Record,
        settings: Option<Settings>,
    ) -> Result<Data, StorageFailure> {
        record.normalize();
        validate_record(&record).map_err(|message| StorageFailure {
            kind: crate::diagnostics::FailureKind::Protocol,
            message,
        })?;
        let mut conflict = false;
        let result = self.update(|d| {
            let known = d
                .active
                .iter()
                .chain(&d.drafts)
                .chain(&d.archives)
                .any(|r| r.id == record.id);
            let born = crate::game::identity_time(&record.id)
                .unwrap_or_else(|| record.created_at.saturating_mul(1_000_000));
            if d.deleted_ids.contains(&record.id)
                || (!known && d.retired_before_nanos > 0 && born <= d.retired_before_nanos)
            {
                conflict = true;
                return Err("此局已被删除，旧窗口不能写回；请另存独立分支".into());
            }
            if record.result == Outcome::Playing && d.archives.iter().any(|r| r.id == record.id) {
                conflict = true;
                return Err("此局已结束，请从复盘续下建立新局".into());
            }
            if let Some(old) = d
                .active
                .iter()
                .chain(d.drafts.iter())
                .chain(d.archives.iter())
                .find(|r| r.id == record.id)
            {
                if !record.acknowledges(old) {
                    conflict = true;
                    return Err("[conflict] 此局已由另一窗口更新，请另存当前分支或重新载入".into());
                }
                record.favorite = old.favorite;
                record.favorite_revision = old.favorite_revision;
                record.title = old.title.clone();
                record.title_revision = old.title_revision;
            }
            if let Some(s) = settings {
                d.settings = s;
            }
            if record.result == Outcome::Playing {
                if let Some(i) = d.drafts.iter().position(|r| r.id == record.id) {
                    d.drafts[i] = record;
                } else if d.active.as_ref().is_none_or(|r| r.id == record.id) {
                    d.active = Some(record);
                } else {
                    if d.drafts.len() >= MAX_DRAFTS {
                        return Err("对局已满，请先结束或删除旧对局".into());
                    }
                    d.drafts.push(d.active.take().unwrap());
                    d.active = Some(record);
                }
            } else {
                if d.active.as_ref().is_some_and(|r| r.id == record.id) {
                    d.active = None;
                }
                d.drafts.retain(|r| r.id != record.id);
                d.archives.retain(|r| r.id != record.id);
                d.archives.insert(0, record);
                d.trim_archives();
            }
            Ok(())
        });
        result.map_err(|message| StorageFailure {
            kind: if conflict {
                crate::diagnostics::FailureKind::Conflict
            } else {
                crate::diagnostics::FailureKind::Storage
            },
            message,
        })
    }
    pub fn activate(&self, id: &str) -> Result<Data, String> {
        self.update(|d| {
            if d.active.as_ref().is_some_and(|r| r.id == id) {
                return Ok(());
            }
            let i = d
                .drafts
                .iter()
                .position(|r| r.id == id)
                .ok_or("对局已不存在")?;
            let r = d.drafts.remove(i);
            if let Some(old) = d.active.replace(r) {
                d.drafts.push(old);
            }
            Ok(())
        })
    }
    pub fn toggle_favorite(&self, time: u64, red: bool) -> Result<Data, String> {
        let d = self.load()?;
        let id = d
            .archives
            .iter()
            .find(|r| r.time == time && r.user_red == red)
            .ok_or("棋谱已不存在")?
            .id
            .clone();
        self.favorite_by_id(&id)
    }
    pub fn favorite_by_id(&self, id: &str) -> Result<Data, String> {
        self.favorite_once(id, &crate::game::new_id())
    }
    pub fn favorite_once(&self, id: &str, operation: &str) -> Result<Data, String> {
        self.update(|d| {
            if d.applied_operations.iter().any(|o| o == operation) {
                return Ok(());
            }
            if d.applied_operations.len() >= MAX_LEGACY_OPERATIONS {
                return Err("旧收藏接口已达到去重容量，请改用带修订号的收藏操作".into());
            }
            let idx = d
                .archives
                .iter()
                .position(|r| r.id == id)
                .ok_or("棋谱已不存在")?;
            if !d.archives[idx].favorite
                && d.archives.iter().filter(|r| r.favorite).count() >= MAX_FAVORITES
            {
                return Err("最多收藏 50 局，请先取消一个收藏".into());
            }
            d.archives[idx].favorite = !d.archives[idx].favorite;
            d.archives[idx].favorite_revision = d.archives[idx]
                .favorite_revision
                .checked_add(1)
                .ok_or("收藏修订号已满")?;
            d.applied_operations.push(operation.to_owned());
            Ok(())
        })
    }
    /// A retry names the desired state and the metadata revision it observed.
    pub fn set_favorite(&self, id: &str, favorite: bool, revision: u64) -> Result<Data, String> {
        self.update(|d| {
            let idx = d
                .archives
                .iter()
                .position(|r| r.id == id)
                .ok_or("棋谱已不存在")?;
            let record = &d.archives[idx];
            if record.favorite_revision != revision {
                if record.favorite == favorite
                    && record.favorite_revision == revision.saturating_add(1)
                {
                    return Ok(());
                }
                return Err("[conflict] 收藏状态已由另一操作更新，请重新读取后操作".into());
            }
            if favorite
                && !record.favorite
                && d.archives.iter().filter(|r| r.favorite).count() >= MAX_FAVORITES
            {
                return Err("最多收藏 50 局，请先取消一个收藏".into());
            }
            d.archives[idx].favorite = favorite;
            d.archives[idx].favorite_revision = revision.checked_add(1).ok_or("收藏修订号已满")?;
            Ok(())
        })
    }
    pub fn remove(&self, time: u64, red: bool) -> Result<Data, String> {
        self.update(|d| {
            d.deleted_ids.extend(
                d.archives
                    .iter()
                    .filter(|r| r.time == time && r.user_red == red)
                    .map(|r| r.id.clone()),
            );
            d.archives.retain(|r| r.time != time || r.user_red != red);
            Ok(())
        })
    }
    pub fn remove_by_id(&self, id: &str) -> Result<Data, String> {
        self.update(|d| {
            if d.archives.iter().chain(&d.drafts).any(|r| r.id == id) {
                d.deleted_ids.insert(id.to_owned());
            }
            d.archives.retain(|r| r.id != id);
            d.drafts.retain(|r| r.id != id);
            Ok(())
        })
    }
    pub fn import_legacy(&self, path: &Path) -> Result<(Data, usize), String> {
        let bytes = Self::read(path)?;
        if bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
            let before = self.load()?;
            let ids: BTreeSet<_> = before
                .active
                .iter()
                .chain(&before.drafts)
                .chain(&before.archives)
                .map(|r| r.id.clone())
                .collect();
            let data = self.import_backup(path)?;
            let added = data
                .active
                .iter()
                .chain(&data.drafts)
                .chain(&data.archives)
                .filter(|r| !ids.contains(&r.id))
                .count();
            return Ok((data, added));
        }
        let mut records: Vec<Record> =
            serde_json::from_slice(&bytes).map_err(|e| format!("旧棋谱格式无效：{e}"))?;
        if records.len() > MAX_IMPORT_RECORDS {
            return Err("单次最多导入 1000 局".into());
        }
        for r in &mut records {
            r.normalize();
            let g = r.replay()?;
            r.result = g.outcome;
            r.end_reason = g.end_reason;
            if r.source == crate::game::RecordSource::Legacy {
                r.id = format!("legacy-{}", r.content_key());
            }
        }
        let mut added = 0;
        let data = self.update(|d| {
            for r in records {
                if d.deleted_ids.contains(&r.id) {
                    continue;
                }
                if d.archives
                    .iter()
                    .chain(d.drafts.iter())
                    .chain(d.active.iter())
                    .any(|g| g.id == r.id && g.content_key() != r.content_key())
                {
                    return Err("[conflict] 导入记录身份相同但内容不同，原存档已保留".into());
                }
                if !d
                    .archives
                    .iter()
                    .chain(d.drafts.iter())
                    .chain(d.active.iter())
                    .any(|g| g.id == r.id)
                {
                    if r.result == Outcome::Playing {
                        if d.drafts.len() >= MAX_DRAFTS {
                            return Err("导入对局超过 50 局上限".into());
                        }
                        d.drafts.push(r);
                    } else {
                        d.archives.push(r);
                    }
                    added += 1;
                }
            }
            d.archives.sort_by_key(|r| std::cmp::Reverse(r.updated_at));
            let favorites = d.archives.iter().filter(|r| r.favorite).count();
            if favorites > MAX_FAVORITES || d.archives.len() - favorites > MAX_ARCHIVES {
                return Err("导入后超过普通棋谱 100 局或收藏 50 局上限；未导入任何记录，现有棋谱与原文件均已保留".into());
            }
            Ok(())
        })?;
        Ok((data, added))
    }
}
