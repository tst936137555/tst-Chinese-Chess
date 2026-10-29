use super::*;
impl App {
    pub(super) fn modal_backups(&mut self, paths: Vec<std::path::PathBuf>, x: f32, y: f32, w: f32) {
        self.center("棋谱备份", self.width / 2., y + 40., 26., INK);
        self.modal = Some(Modal::Backups(paths.clone()));
        if self.button(
            "导出当前棋谱备份",
            Rect::new(
                x + 24.,
                y + if self.height < 500. { 56. } else { 70. },
                w - 48.,
                if self.height < 500. { 40. } else { 44. },
            ),
            true,
            self.saves.pending == 0 && self.saves.failed.is_empty(),
        ) {
            self.enqueue(Command::Export);
        }
        self.fit_text(
            "选择备份可合并恢复，当前棋谱保留",
            x + 24.,
            y + if self.height < 500. { 120. } else { 143. },
            w - 48.,
            15.,
            MUTED,
        );
        let compact = self.height < 500.;
        let count = if compact { 2 } else { 6 };
        let pages = paths.len().max(1).div_ceil(count);
        let mut page = (self.modal_scroll as usize).min(pages - 1);
        let wheel = self.wheel().1;
        if wheel != 0. && Rect::new(x, y, w, self.height).contains(self.pointer()) {
            page = page
                .saturating_add_signed(if wheel < 0. { 1 } else { -1 })
                .min(pages - 1);
            self.modal_scroll = page as f32;
        }
        for (i, path) in paths.iter().skip(page * count).take(count).enumerate() {
            let date = std::fs::metadata(path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_millis() as u64);
            if self.button(
                &format!(
                    "{} · {}",
                    if path
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with("export"))
                    {
                        "导出"
                    } else {
                        "自动"
                    },
                    super::super::screens::date_label(date)
                ),
                Rect::new(
                    x + 24.,
                    y + if compact { 132. } else { 165. } + i as f32 * 54.,
                    w - 48.,
                    44.,
                ),
                false,
                true,
            ) {
                self.open_modal(Modal::ImportBackup(path.clone()));
            }
        }
        for (i, label, target, enabled) in [
            (0, "较新备份", page.saturating_sub(1), page > 0),
            (1, "更早备份", page + 1, page + 1 < pages),
        ] {
            if self.button(
                label,
                Rect::new(
                    x + 24. + i as f32 * (w - 48.) / 2.,
                    y + if compact { 242. } else { 500. },
                    (w - 56.) / 2.,
                    36.,
                ),
                false,
                enabled,
            ) {
                self.modal_scroll = target as f32;
            }
        }
        if paths.is_empty() {
            self.center("暂无备份", self.width / 2., y + 230., 20., MUTED);
        }
        if self.button(
            "返回",
            Rect::new(x + 24., y + if compact { 286. } else { 550. }, w - 48., 40.),
            false,
            true,
        ) {
            self.modal = None;
        }
    }
    pub(super) fn modal_import_backup(&mut self, path: std::path::PathBuf, x: f32, y: f32, w: f32) {
        self.center("合并恢复这份备份？", self.width / 2., y + 44., 24., INK);
        self.modal_text(
            "现有棋谱和设置保留。内容冲突或超出容量时将取消本次恢复。",
            x + 24.,
            y + 80.,
            w - 48.,
            100.,
            17.,
        );
        self.modal = Some(Modal::ImportBackup(path.clone()));
        if self.button(
            "确认合并恢复",
            Rect::new(x + 24., y + 210., w - 48., 48.),
            true,
            true,
        ) {
            self.enqueue(Command::ImportBackup(path));
            self.modal = None;
        }
        if self.button(
            "取消",
            Rect::new(x + 24., y + 280., w - 48., 44.),
            false,
            true,
        ) {
            self.enqueue(Command::Backups);
        }
    }
    pub(super) fn modal_save_failed(&mut self, x: f32, y: f32, w: f32) {
        let (title, explanation) = self.saves.failure_description();
        self.center(title, self.width / 2., y + 40., 26., INK);
        self.modal = Some(Modal::SaveFailed);
        self.modal_text(explanation, x + 24., y + 75., w - 48., 60., 17.);
        if self.button(
            "重试保存",
            Rect::new(x + 24., y + 145., w - 48., 48.),
            true,
            !self.saves.failed.is_empty(),
        ) {
            self.notice.clear();
            for command in std::mem::take(&mut self.saves.failed) {
                self.enqueue(command);
            }
            self.modal = None;
        }
        if self.button(
            "另存后退出",
            Rect::new(x + 24., y + 205., w - 48., 48.),
            false,
            self.saves
                .failed
                .iter()
                .any(|c| matches!(c, Command::Save(..))),
        ) {
            self.recover_branches();
            self.close.request();
            self.enqueue(Command::Flush);
            self.modal = None;
        }
        if self.button(
            "返回对局",
            Rect::new(x + 24., y + 285., w - 48., 44.),
            false,
            true,
        ) {
            self.modal = None;
        }
        if self.button(
            "放弃未保存内容并退出",
            Rect::new(x + 24., y + 345., w - 48., 44.),
            false,
            true,
        ) {
            self.close = crate::session::CloseState::Forced;
        }
    }
    pub(super) fn modal_storage_recovery(&mut self, x: f32, y: f32, w: f32) {
        self.center("存档暂时无法读取", self.width / 2., y + 40., 26., INK);
        self.modal_text(
            "可恢复备份，或隔离损坏棋谱。\n恢复前会保留原文件。",
            x + 24.,
            y + 80.,
            w - 48.,
            65.,
            17.,
        );
        self.modal = Some(Modal::StorageRecovery);
        if self.button(
            "重试读取",
            Rect::new(x + 24., y + 160., w - 48., 48.),
            true,
            true,
        ) {
            self.enqueue(Command::Load);
        }
        if self.button(
            "恢复上次备份",
            Rect::new(x + 24., y + 220., w - 48., 48.),
            false,
            true,
        ) {
            self.enqueue(Command::RestoreBackup);
        }
        if self.button(
            "隔离坏局并恢复",
            Rect::new(x + 24., y + 275., w - 48., 44.),
            false,
            true,
        ) {
            self.enqueue(Command::Repair);
        }
        if self.button(
            "暂不恢复",
            Rect::new(x + 24., y + 330., w - 48., 48.),
            false,
            true,
        ) {
            self.modal = None;
        }
    }
}
