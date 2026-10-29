use super::*;
impl App {
    pub(super) fn modal_clear_drafts(&mut self, x: f32, y: f32, w: f32) {
        self.center("删除全部旧对局？", self.width / 2., y + 44., 25., INK);
        self.center("保留当前活动对局", self.width / 2., y + 103., 17., MUTED);
        self.modal = Some(Modal::ClearDrafts);
        if self.button(
            "确认删除旧对局",
            Rect::new(x + 24., y + 160., w - 48., 48.),
            true,
            true,
        ) {
            self.enqueue(Command::RemoveChecked(self.delete_targets.clone()));
            self.open_modal(Modal::Drafts(0));
        }
        if self.button(
            "保留对局",
            Rect::new(x + 24., y + 230., w - 48., 44.),
            false,
            true,
        ) {
            self.open_modal(Modal::Drafts(0));
        }
    }
    pub(super) fn modal_drafts(&mut self, page: usize, x: f32, y: f32, w: f32) {
        self.center("继续未完成对局", self.width / 2., y + 40., 26., INK);
        let records: Vec<_> = self
            .data
            .active
            .iter()
            .chain(self.data.drafts.iter())
            .cloned()
            .collect();
        if records.is_empty() {
            self.center("暂无未完成的对局", self.width / 2., y + 115., 21., MUTED);
            self.center(
                "开启新局后，每一步都会自动保存",
                self.width / 2.,
                y + 155.,
                15.,
                MUTED,
            );
            if self.button(
                "返回",
                Rect::new(x + 24., y + 225., w - 48., 44.),
                false,
                true,
            ) {
                self.modal = None;
            } else {
                self.modal = Some(Modal::Drafts(0));
            }
        } else {
            let compact = self.height < 500.;
            let rows = if compact { 2 } else { 5 };
            let step = if compact { 76. } else { 73. };
            let pages = records.len().max(1).div_ceil(rows);
            let page = page.min(pages - 1);
            self.modal = Some(Modal::Drafts(page));
            for (i, r) in records.iter().skip(page * rows).take(rows).enumerate() {
                self.semantic_scope = format!("draft:{}", r.id);
                let label = format!(
                    "{} · {} · {}步",
                    if r.user_red { "执红" } else { "执黑" },
                    r.level_name,
                    r.history.len()
                );
                if self.button_named(
                    "resume",
                    &label,
                    Rect::new(x + 24., y + 80. + i as f32 * step, w - 160., 40.),
                    false,
                    true,
                ) {
                    self.resume_record(r.clone());
                    break;
                }
                self.fit_text(
                    &format!(
                        "{} · {}",
                        if r.title.is_empty() {
                            "未命名"
                        } else {
                            &r.title
                        },
                        super::screens::date_label(r.updated_at)
                    ),
                    x + 28.,
                    y + 150. + i as f32 * step,
                    w - 56.,
                    15.,
                    MUTED,
                );
                if self.button(
                    "命名",
                    Rect::new(x + w - 128., y + 80. + i as f32 * step, 50., 40.),
                    false,
                    true,
                ) {
                    self.open_modal(Modal::Rename(r.id.clone(), r.title.clone().into()));
                }
                if self.button(
                    "删除",
                    Rect::new(x + w - 72., y + 80. + i as f32 * step, 48., 40.),
                    false,
                    self.data.active.as_ref().is_none_or(|a| a.id != r.id),
                ) {
                    self.open_modal(Modal::Delete(r.id.clone()));
                }
            }
            self.semantic_scope.clear();
            if self.button(
                "清理旧对局",
                Rect::new(
                    x + 24.,
                    y + if compact { 286. } else { 510. },
                    if compact { (w - 56.) / 2. } else { w - 48. },
                    40.,
                ),
                false,
                !self.data.drafts.is_empty(),
            ) {
                self.open_modal(Modal::ClearDrafts);
            }
            if self.button(
                "上一页",
                Rect::new(
                    x + 24.,
                    y + if compact { 236. } else { 460. },
                    (w - 56.) / 2.,
                    40.,
                ),
                false,
                page > 0,
            ) {
                self.modal = Some(Modal::Drafts(page - 1));
            }
            if self.button(
                "下一页",
                Rect::new(
                    x + w / 2. + 4.,
                    y + if compact { 236. } else { 460. },
                    (w - 56.) / 2.,
                    40.,
                ),
                false,
                page + 1 < pages,
            ) {
                self.modal = Some(Modal::Drafts(page + 1));
            }
            if self.button(
                "取消",
                Rect::new(
                    if compact { x + w / 2. + 4. } else { x + 24. },
                    y + if compact { 286. } else { 556. },
                    if compact { (w - 56.) / 2. } else { w - 48. },
                    40.,
                ),
                false,
                true,
            ) {
                self.modal = None;
            }
        }
    }
    pub(super) fn modal_delete(&mut self, id: String, x: f32, y: f32, w: f32) {
        self.center("删除这局棋谱？", self.width / 2., y + 44., 26., INK);
        self.center("删除后无法恢复", self.width / 2., y + 103., 18., MUTED);
        self.modal = Some(Modal::Delete(id.clone()));
        if self.button(
            "确认删除",
            Rect::new(x + 24., y + 155., w - 48., 49.),
            true,
            true,
        ) {
            self.enqueue(Command::RemoveChecked(self.delete_targets.clone()));
            self.modal = None;
        }
        if self.button(
            "保留棋谱",
            Rect::new(x + 24., y + 225., w - 48., 44.),
            false,
            true,
        ) {
            self.modal = None;
        }
    }
    pub(super) fn modal_clear(&mut self, x: f32, y: f32, w: f32) {
        self.center("清空全部棋谱？", self.width / 2., y + 44., 26., INK);
        self.center(
            "包含收藏的棋谱，清空后无法恢复",
            self.width / 2.,
            y + 103.,
            17.,
            MUTED,
        );
        self.modal = Some(Modal::Clear);
        if self.button(
            "确认清空",
            Rect::new(x + 24., y + 155., w - 48., 49.),
            true,
            true,
        ) {
            self.enqueue(Command::RemoveChecked(self.delete_targets.clone()));
            self.modal = None;
        }
        if self.button(
            "保留棋谱",
            Rect::new(x + 24., y + 225., w - 48., 44.),
            false,
            true,
        ) {
            self.modal = None;
        }
    }
}
