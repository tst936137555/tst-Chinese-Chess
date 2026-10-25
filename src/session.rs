//! Pure save acknowledgement and task recovery state, independent of rendering.
use crate::{game::Record, persistence::Command, storage::Data};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CloseState {
    #[default]
    Open,
    Waiting {
        sequence: Option<u64>,
        flushed: bool,
    },
    Forced,
}
impl CloseState {
    pub fn request(&mut self) {
        *self = Self::Waiting {
            sequence: None,
            flushed: false,
        };
    }
    pub fn waiting(self) -> bool {
        matches!(self, Self::Waiting { .. })
    }
    pub fn queued_flush(&mut self, seq: u64) {
        if let Self::Waiting { sequence: None, .. } = self {
            *self = Self::Waiting {
                sequence: Some(seq),
                flushed: false,
            };
        }
    }
    pub fn flushed(&mut self, seq: u64) {
        if let Self::Waiting {
            sequence: Some(expected),
            flushed,
        } = self
            && *expected == seq
        {
            *flushed = true;
        }
    }
    pub fn cancel(&mut self) {
        *self = Self::Open;
    }
    pub fn can_exit(self, pending: usize, failed: bool) -> bool {
        self == Self::Forced
            || (matches!(self, Self::Waiting { flushed: true, .. }) && pending == 0 && !failed)
    }
}
#[derive(Default)]
pub struct SaveState {
    pub last_request: u64,
    pub pending: usize,
    pub failed: Vec<Command>,
    started: std::collections::BTreeMap<u64, std::time::Instant>,
    commands: std::collections::BTreeMap<u64, Command>,
}
impl SaveState {
    pub fn project_preferences(
        &self,
        mut settings: crate::storage::Settings,
    ) -> crate::storage::Settings {
        for command in self.failed.iter().chain(self.commands.values()) {
            match command {
                Command::SettingsPatch(change) => change.apply(&mut settings),
                Command::Settings(value) | Command::Save(_, Some(value)) => {
                    settings = value.clone()
                }
                _ => {}
            }
        }
        settings
    }
    /// Keep confirmed metadata visible while overlaying every uncommitted board.
    /// A failed write remains recoverable even when a later disk read succeeds.
    pub fn project(&self, mut data: Data) -> Data {
        for command in self.failed.iter().chain(self.commands.values()) {
            if let Command::Save(record, _) = command {
                overlay_record(&mut data, record);
            }
        }
        data
    }

    pub fn status(&self, id: &str) -> SaveStatus {
        if self.failed.iter().any(|c| match c {
            Command::Save(r, _) => r.id == id,
            _ => false,
        }) {
            SaveStatus::Failed
        } else if self.commands.values().any(|c| match c {
            Command::Save(r, _) => r.id == id,
            _ => false,
        }) {
            SaveStatus::Pending
        } else {
            SaveStatus::Saved
        }
    }
    pub fn queued(&mut self, seq: u64, command: Command) {
        if let Command::Rename(id, _, revision) = &command {
            self.failed.retain(|old| {
                !matches!(old, Command::Rename(old_id, _, old_revision)
                if old_id == id && old_revision <= revision)
            });
        }
        if let Command::SetFavorite { id, revision, .. } = &command {
            self.failed.retain(|old| !matches!(old,Command::SetFavorite {id:old_id,revision:old_revision,..} if old_id==id && old_revision<=revision));
        }
        self.last_request = seq;
        self.pending += 1;
        self.started.insert(seq, std::time::Instant::now());
        self.commands.insert(seq, command);
    }
    pub fn completed(&mut self, seq: u64) -> Option<Command> {
        self.pending = self.pending.saturating_sub(1);
        if let Some(start) = self.started.remove(&seq) {
            crate::diagnostics::measure(
                "persistence_ack_ms",
                start.elapsed().as_secs_f64() * 1000.,
            );
        }
        self.commands.remove(&seq)
    }
    pub fn succeeded(&mut self, seq: u64, data: &Data) {
        let command = self.completed(seq);
        if let Some(Command::SettingsPatch(change)) = &command {
            self.supersede_preferences(change);
        }
        if matches!(
            command,
            Some(Command::Settings(_) | Command::Save(_, Some(_)))
        ) {
            // A newer successful preference write supersedes preferences bundled
            // with an older failed board write, while retaining its unsaved moves.
            for failed in &mut self.failed {
                if let Command::Save(_, settings) = failed {
                    *settings = None;
                }
            }
        }
        self.failed.retain(|failed| {
            !matches!(
                (&command, failed),
                (
                    Some(Command::Settings(_) | Command::Save(_, Some(_))),
                    Command::Settings(_)
                ) | (
                    Some(Command::Load | Command::RestoreBackup),
                    Command::Load | Command::RestoreBackup
                )
            )
        });
        self.acknowledge(data);
    }
    pub fn acknowledge(&mut self, data: &Data) {
        self.failed.retain(|command| match command {
            Command::Save(r, settings) => {
                !(data
                    .active
                    .iter()
                    .chain(data.drafts.iter())
                    .chain(data.archives.iter())
                    .any(|saved| saved.acknowledges(r))
                    && settings.as_ref().is_none_or(|s| *s == data.settings))
            }
            Command::Settings(settings) => *settings != data.settings,
            Command::SettingsPatch(change) => !change.acknowledged(&data.settings),
            Command::SetFavorite {
                id,
                favorite,
                revision,
            } => !data.archives.iter().any(|r| {
                r.id == *id
                    && r.favorite == *favorite
                    && r.favorite_revision == revision.saturating_add(1)
            }),
            Command::Rename(id, title, revision) => !data
                .active
                .iter()
                .chain(&data.drafts)
                .chain(&data.archives)
                .any(|r| {
                    r.id == *id
                        && r.title == title.trim()
                        && r.title_revision == revision.saturating_add(1)
                }),
            _ => true,
        });
    }
    pub fn failed(&mut self, command: Command) {
        if let Command::Rename(id, _, revision) = &command {
            if self.commands.values().any(|newer| {
                matches!(newer, Command::Rename(new_id, _, new_revision)
                if new_id == id && new_revision >= revision)
            }) {
                return;
            }
            self.failed.retain(|old| {
                !matches!(old, Command::Rename(old_id, _, old_revision)
                if old_id == id && old_revision <= revision)
            });
        }
        if let Command::SetFavorite { id, revision, .. } = &command {
            if self.commands.values().any(|newer|matches!(newer,Command::SetFavorite {id:new_id,revision:new_revision,..} if new_id==id && new_revision>=revision)) {
                return;
            }
            self.failed.retain(|old|!matches!(old,Command::SetFavorite {id:old_id,revision:old_revision,..} if old_id==id && old_revision<=revision));
        }
        if let Command::SettingsPatch(change) = &command {
            self.supersede_preferences(change);
        }
        if matches!(command, Command::Settings(_)) {
            self.failed
                .retain(|old| !matches!(old, Command::Settings(_)));
        }
        if let Command::Save(r, _) = &command {
            self.failed
                .retain(|old| !matches!(old,Command::Save(prior,_) if r.acknowledges(prior)));
        }
        self.failed.push(command);
    }
    fn supersede_preferences(&mut self, change: &crate::storage::SettingsChange) {
        for command in &mut self.failed {
            if let Command::SettingsPatch(old) = command {
                old.supersede(change);
            }
        }
        self.failed
            .retain(|c| !matches!(c, Command::SettingsPatch(change) if change.is_empty()));
    }
    pub fn unsaved(&self, id: &str) -> Option<&Record> {
        self.failed.iter().rev().find_map(|c| match c {
            Command::Save(r, _) if r.id == id => Some(r.as_ref()),
            _ => None,
        })
    }
    pub fn failure_description(&self) -> (&'static str, &'static str) {
        let has = |predicate: fn(&Command) -> bool| self.failed.iter().any(predicate);
        if has(|c| matches!(c, Command::Save(..))) {
            ("对局尚未保存", "当前棋盘仍保留在内存中，可重试或另存分支。")
        } else if has(|c| matches!(c, Command::Settings(_) | Command::SettingsPatch(_))) {
            ("声音或偏好设置未保存", "对局内容不受影响，可重试保存设置。")
        } else if has(|c| matches!(c, Command::Rename(..))) {
            ("名称尚未保存", "对局内容不受影响，可重试保存名称。")
        } else if has(|c| matches!(c, Command::Load | Command::RestoreBackup)) {
            ("存档尚未读取", "当前棋盘已保留，可重试读取或恢复备份。")
        } else {
            (
                "棋谱变更尚未保存",
                "可重试收藏或删除等操作，已确认的对局内容仍保留。",
            )
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveStatus {
    Pending,
    Saved,
    Failed,
}

/// Return all unacknowledged histories; matching identities alone prove nothing.
pub fn recovery_records(failed: &Record, current: Option<&Record>) -> Vec<Record> {
    let Some(current) = current.filter(|r| r.id == failed.id) else {
        return vec![failed.clone()];
    };
    if current.acknowledges(failed) {
        vec![current.clone()]
    } else if failed.acknowledges(current) {
        vec![failed.clone()]
    } else {
        vec![failed.clone(), current.clone()]
    }
}

fn overlay_record(data: &mut Data, record: &Record) {
    let old = data
        .active
        .iter()
        .chain(&data.drafts)
        .chain(&data.archives)
        .find(|r| r.id == record.id);
    if old.is_some_and(|r| r.acknowledges(record)) {
        return;
    }
    let mut record = record.clone();
    if let Some(old) = old {
        record.title = old.title.clone();
        record.title_revision = old.title_revision;
        record.favorite = old.favorite;
        record.favorite_revision = old.favorite_revision;
    }
    if record.result == crate::game::Outcome::Playing {
        if let Some(old) = data.active.as_mut().filter(|r| r.id == record.id) {
            *old = record;
        } else if let Some(old) = data.drafts.iter_mut().find(|r| r.id == record.id) {
            *old = record;
        } else {
            if let Some(old) = data.active.replace(record) {
                data.drafts.push(old);
            }
        }
    } else {
        if data.active.as_ref().is_some_and(|r| r.id == record.id) {
            data.active = None;
        }
        data.drafts.retain(|r| r.id != record.id);
        data.archives.retain(|r| r.id != record.id);
        data.archives.insert(0, record);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskKind {
    Move,
    Hint,
    Review,
    End,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineTask {
    Move,
    Hint,
    Review(usize),
    End,
}
impl EngineTask {
    pub fn kind(self) -> TaskKind {
        match self {
            Self::Move => TaskKind::Move,
            Self::Hint => TaskKind::Hint,
            Self::Review(_) => TaskKind::Review,
            Self::End => TaskKind::End,
        }
    }
}
/// Presentation pacing is independent of engine strength and search time limits.
pub fn minimum_think_seconds(level: usize) -> f64 {
    [0.18, 0.30, 0.45, 0.80, 1.0][level.min(4)]
}
#[derive(Default)]
pub struct Recovery {
    pub resume: Option<TaskKind>,
}
impl Recovery {
    pub fn pause(&mut self, kind: Option<TaskKind>) {
        if kind.is_some() {
            self.resume = kind;
        }
    }
    pub fn resume(&mut self, suspended: bool) -> Option<TaskKind> {
        if suspended { None } else { self.resume.take() }
    }
}
