//! Versioned, atomic storage. UI writes run on a serial worker; all transactions lock on disk.
use crate::game::{Outcome, Record};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub const SCHEMA_VERSION: u32 = 4;
const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_IMPORT_RECORDS: usize = 1000;
const MAX_DRAFTS: usize = 50;

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
            extra: BTreeMap::new(),
        }
    }
}
impl Data {
    fn validate(&self) -> Result<(), String> {
        if self.archives.len() > MAX_IMPORT_RECORDS || self.drafts.len() > MAX_DRAFTS {
            return Err("存档局数超限，原文件已保留".into());
        }
        for record in self.active.iter().chain(&self.archives).chain(&self.drafts) {
            record.validate_metadata()?;
        }
        Ok(())
    }
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
        d.applied_operations.truncate(128);
        d.settings.sanitize();
        d.version = SCHEMA_VERSION;
        Ok(d)
    }
    pub fn load(&self) -> Result<Data, String> {
        match fs::metadata(&self.path) {
            Ok(_) => Self::decode(&Self::read(&self.path)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Data::default()),
            Err(e) => Err(e.to_string()),
        }
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
                Ok(()) => return Ok(lock),
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
        tmp.write_all(bytes)
            .and_then(|()| tmp.as_file().sync_all())
            .map_err(|e| e.to_string())?;
        tmp.persist(path).map_err(|e| e.to_string())?;
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
        op(&mut data)?;
        data.version = SCHEMA_VERSION;
        data.settings.sanitize();
        data.validate()?;
        let bytes = serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err("存档超过 32 MiB 限制".into());
        }
        if self.path.is_file() {
            Self::atomic_write(&self.path.with_extension("bak"), &Self::read(&self.path)?)?;
        }
        Self::atomic_write(&self.path, &bytes)?;
        Ok(data)
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
        let data = Self::decode(&bytes)?;
        Self::atomic_write(&self.path, &bytes)?;
        Ok(data)
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
        record.replay().map_err(|message| StorageFailure {
            kind: crate::diagnostics::FailureKind::Protocol,
            message,
        })?;
        let mut conflict = false;
        let result = self.update(|d| {
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
                record.title = old.title.clone();
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
                trim(&mut d.archives);
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
            let idx = d
                .archives
                .iter()
                .position(|r| r.id == id)
                .ok_or("棋谱已不存在")?;
            if !d.archives[idx].favorite && d.archives.iter().filter(|r| r.favorite).count() >= 50 {
                return Err("最多收藏 50 局，请先取消一个收藏".into());
            }
            d.archives[idx].favorite = !d.archives[idx].favorite;
            d.applied_operations.push(operation.to_owned());
            if d.applied_operations.len() > 128 {
                d.applied_operations.remove(0);
            }
            Ok(())
        })
    }
    pub fn remove(&self, time: u64, red: bool) -> Result<Data, String> {
        self.update(|d| {
            d.archives.retain(|r| r.time != time || r.user_red != red);
            Ok(())
        })
    }
    pub fn remove_by_id(&self, id: &str) -> Result<Data, String> {
        self.update(|d| {
            d.archives.retain(|r| r.id != id);
            d.drafts.retain(|r| r.id != id);
            Ok(())
        })
    }
    pub fn import_legacy(&self, path: &Path) -> Result<(Data, usize), String> {
        let mut records: Vec<Record> = serde_json::from_slice(&Self::read(path)?)
            .map_err(|e| format!("旧棋谱格式无效：{e}"))?;
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
            trim(&mut d.archives);
            Ok(())
        })?;
        Ok((data, added))
    }
}
fn trim(records: &mut Vec<Record>) {
    let (mut plain, mut fav) = (0, 0);
    records.retain(|r| {
        let n = if r.favorite { &mut fav } else { &mut plain };
        *n += 1;
        *n <= if r.favorite { 50 } else { 100 }
    });
}
