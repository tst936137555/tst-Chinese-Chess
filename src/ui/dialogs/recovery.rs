use super::*;
impl App {
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
            "可重试读取，或恢复上次备份。\n恢复时会另存当前文件。",
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
            "暂不恢复",
            Rect::new(x + 24., y + 330., w - 48., 48.),
            false,
            true,
        ) {
            self.modal = None;
        }
    }
}
