use super::*;

impl App {
    pub(super) fn notice_contains(&self, point: Option<Vec2>) -> bool {
        if self.saves.failed.is_empty() || self.modal.is_some() {
            return false;
        }
        let Some(point) = point else { return false };
        let button = crate::interaction::touch_area(
            [self.width - 104., self.height - 50., 92., 44.],
            self.width / self.viewport.w,
        );
        point.y >= self.height - 56.
            || Rect::new(button[0], button[1], button[2], button[3]).contains(point)
    }

    pub(super) fn draw_notice(&mut self) {
        if !self.saves.failed.is_empty() && self.modal.is_none() {
            draw_rectangle(0., self.height - 56., self.width, 56., PAPER);
            self.fit_text(
                "保存失败，修改仍保留",
                12.,
                self.height - 22.,
                self.width - 124.,
                15.,
                RED_INK,
            );
            if self.button(
                "处理保存",
                Rect::new(self.width - 104., self.height - 50., 92., 44.),
                false,
                true,
            ) {
                self.open_modal(Modal::SaveFailed);
            }
        } else if !self.notice.is_empty() {
            draw_rectangle(0., self.height - 25., self.width, 25., PAPER);
            self.fit_text(
                &self.notice,
                12.,
                self.height - 7.,
                self.width - 24.,
                14.,
                RED_INK,
            );
        }
    }
}
