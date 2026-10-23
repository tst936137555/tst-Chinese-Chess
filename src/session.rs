//! Pure save acknowledgement and task recovery state, independent of rendering.
use crate::{game::Record, persistence::Command, storage::Data};
#[derive(Default)]
pub struct SaveState {
    pub last_request: u64,
    pub pending: usize,
    pub failed: Vec<Command>,
    started: std::collections::BTreeMap<u64, std::time::Instant>,
    commands: std::collections::BTreeMap<u64, Command>,
}
impl SaveState {
    pub fn queued(&mut self, seq: u64, command: Command) {
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
            Command::Rename(id, title) => !data
                .active
                .iter()
                .chain(&data.drafts)
                .chain(&data.archives)
                .any(|r| r.id == *id && r.title == title.trim()),
            _ => true,
        });
    }
    pub fn failed(&mut self, command: Command) {
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
    pub fn unsaved(&self, id: &str) -> Option<&Record> {
        self.failed.iter().rev().find_map(|c| match c {
            Command::Save(r, _) if r.id == id => Some(r.as_ref()),
            _ => None,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskKind {
    Move,
    Hint,
    Review,
    End,
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
