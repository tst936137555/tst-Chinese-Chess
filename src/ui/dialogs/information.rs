use super::*;
impl App {
    pub(super) fn modal_review_moves(&mut self, page: usize, x: f32, y: f32, w: f32) {
        let Some(review) = &self.review else {
            self.modal = None;
            return;
        };
        let compact = self.height < 500.;
        let count = if compact { 2 } else { 8 };
        let pages = review.game.history.len().max(1).div_ceil(count);
        let page = page.min(pages - 1);
        let rows: Vec<_> = review
            .game
            .history
            .iter()
            .enumerate()
            .skip(page * count)
            .take(count)
            .map(|(i, e)| {
                (
                    i + 1,
                    format!("{}. {} · {}", i + 1, e.mover.label(), e.notation),
                )
            })
            .collect();
        let cursor = review.ply;
        let mistakes = review.mistake_plies();
        self.modal = Some(Modal::ReviewMoves(page));
        let wheel = self.wheel().1;
        if wheel != 0. && Rect::new(x, y, w, self.height).contains(self.pointer()) {
            let target = page
                .saturating_add_signed(if wheel < 0. { 1 } else { -1 })
                .min(pages - 1);
            self.modal = Some(Modal::ReviewMoves(target));
        }
        self.center("本局棋谱", self.width / 2., y + 38., 26., INK);
        if self.button(
            "翻转",
            Rect::new(x + w - 84., y + 12., 60., 38.),
            false,
            true,
        ) {
            let r = self.review.as_mut().unwrap();
            r.flipped = !r.flipped;
            self.modal = None;
        }
        for (row, (ply, label)) in rows.into_iter().enumerate() {
            if self.button(
                &label,
                Rect::new(
                    x + 24.,
                    y + 68. + row as f32 * if compact { 60. } else { 48. },
                    w - 48.,
                    40.,
                ),
                ply == cursor,
                true,
            ) {
                self.review.as_mut().unwrap().ply = ply;
                self.modal = None;
            }
        }
        for (i, label, target) in [
            (
                0,
                "上处失误",
                mistakes.iter().copied().rev().find(|&p| p < cursor),
            ),
            (
                1,
                "下处失误",
                mistakes.iter().copied().find(|&p| p > cursor),
            ),
        ] {
            if self.button(
                label,
                Rect::new(
                    x + 24. + i as f32 * (w - 48.) / 2.,
                    y + if compact { 192. } else { 460. },
                    (w - 56.) / 2.,
                    40.,
                ),
                false,
                target.is_some(),
            ) {
                self.review.as_mut().unwrap().ply = target.unwrap();
                self.modal = None;
            }
        }
        for (i, label, target, enabled) in [
            (0, "上一页", page.saturating_sub(1), page > 0),
            (1, "下一页", page + 1, page + 1 < pages),
        ] {
            if self.button(
                label,
                Rect::new(
                    x + 24. + i as f32 * (w - 48.) / 2.,
                    y + if compact { 242. } else { 510. },
                    (w - 56.) / 2.,
                    36.,
                ),
                false,
                enabled,
            ) {
                self.modal = Some(Modal::ReviewMoves(target));
            }
        }
        if self.button(
            "返回复盘",
            Rect::new(x + 24., y + if compact { 286. } else { 557. }, w - 48., 36.),
            false,
            true,
        ) {
            self.modal = None;
        }
    }
    pub(super) fn modal_variation(&mut self, text: String, x: f32, y: f32, w: f32) {
        let compact = self.height < 500.;
        self.center("参考变化", self.width / 2., y + 38., 26., INK);
        self.modal_text(
            &text,
            x + 24.,
            y + if compact { 70. } else { 80. },
            w - 48.,
            if compact { 128. } else { 345. },
            18.,
        );
        self.modal = Some(Modal::Variation(text));
        if self.button(
            "重新分析",
            Rect::new(x + 24., y + if compact { 214. } else { 455. }, w - 48., 44.),
            false,
            self.pending.is_none(),
        ) && let Some(ply) = self.review.as_ref().map(|r| r.ply)
        {
            self.modal = None;
            self.engine_failed = false;
            self.search(Job::Review(ply));
        }
        if self.button(
            "返回复盘",
            Rect::new(
                x + 24.,
                y + if self.height < 500. { 274. } else { 520. },
                w - 48.,
                44.,
            ),
            true,
            true,
        ) {
            self.modal = None;
        }
    }
    pub(super) fn modal_help(&mut self, x: f32, y: f32, w: f32) {
        self.center("规则说明", self.width / 2., y + 38., 26., INK);
        let body = super::copy::HELP;
        self.modal_text(
            body,
            x + 22.,
            y + if self.height < 500. { 70. } else { 80. },
            w - 44.,
            if self.height < 500. { 190. } else { 400. },
            18.,
        );
        self.modal = Some(Modal::Help);
        if self.button(
            "我知道了",
            Rect::new(
                x + 24.,
                y + if self.height < 500. { 274. } else { 520. },
                w - 48.,
                44.,
            ),
            true,
            true,
        ) {
            self.modal = None;
        }
    }
    pub(super) fn modal_about(&mut self, x: f32, y: f32, w: f32) {
        self.center("作者声明", self.width / 2., y + 36., 26., INK);
        let body = super::copy::ABOUT;
        self.modal_text(
            body,
            x + 22.,
            y + if self.height < 500. { 65. } else { 75. },
            w - 44.,
            if self.height < 500. { 110. } else { 325. },
            17.,
        );
        self.modal = Some(Modal::About);
        for (i, label) in ["GPLv3 全文", "NNUE 许可证", "字体 OFL 全文"]
            .iter()
            .enumerate()
        {
            if self.button(
                label,
                if self.height < 500. {
                    Rect::new(
                        x + 24. + i as f32 * (w - 40.) / 3.,
                        y + 194.,
                        (w - 64.) / 3.,
                        50.,
                    )
                } else {
                    Rect::new(x + 24., y + 415. + i as f32 * 43., w - 48., 36.)
                },
                false,
                true,
            ) {
                self.open_modal(Modal::License(i));
            }
        }
        if self.button(
            "确认",
            Rect::new(
                x + 24.,
                y + if self.height < 500. { 278. } else { 550. },
                w - 48.,
                44.,
            ),
            true,
            true,
        ) {
            self.modal = None;
        }
    }
    pub(super) fn modal_license(&mut self, index: usize, x: f32, y: f32, w: f32) {
        self.center(
            ["GNU GPL v3.0", "NNUE License", "SIL OFL 1.1"][index],
            self.width / 2.,
            y + 35.,
            24.,
            INK,
        );
        let body = self.license_texts[index].clone();
        self.modal_text(
            &body,
            x + 20.,
            y + if self.height < 500. { 65. } else { 75. },
            w - 40.,
            if self.height < 500. { 196. } else { 425. },
            15.,
        );
        self.modal = Some(Modal::License(index));
        if self.button(
            "返回声明",
            Rect::new(
                x + 24.,
                y + if self.height < 500. { 278. } else { 530. },
                w - 48.,
                44.,
            ),
            false,
            true,
        ) {
            self.open_modal(Modal::About);
        }
    }
    pub(in crate::ui) fn modal_text(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        size: f32,
    ) {
        self.semantic(text.to_owned(), Rect::new(x, y, w, h), false);
        let text_width = (w - 22.).max(1.);
        let cache_key = format!("{text_width}:{size}:{}:{text}", self.font.raster_key(size));
        if !self.text_cache.contains_key(&cache_key) {
            let mut lines = Vec::new();
            for paragraph in text.lines() {
                let mut line = String::new();
                for ch in paragraph.chars() {
                    if self
                        .font
                        .measure(&(line.clone() + &ch.to_string()), size)
                        .width
                        > text_width
                    {
                        // Keep closing punctuation with the preceding character.
                        // Otherwise a full-width stop can occupy an entire line by itself.
                        let carry = if "，。、；：？！）】》」』,.!?;:)]".contains(ch) {
                            line.pop()
                        } else {
                            None
                        };
                        lines.push(line);
                        line = String::new();
                        if let Some(c) = carry {
                            line.push(c);
                        }
                    }
                    line.push(ch);
                }
                lines.push(line);
            }
            if self.text_cache.len() > 32 {
                self.text_cache.clear();
            }
            self.text_cache.insert(cache_key.clone(), lines);
        }
        let line_height = size * 1.5;
        let content_height = self.text_cache[&cache_key].len() as f32 * line_height;
        let bar = crate::interaction::Scrollbar::new(h, content_height, self.modal_scroll);
        let content = Rect::new(x, y, text_width, h);
        let track = Rect::new(x + w - 10., y, 8., h);
        let point = self.pointer();
        if Rect::new(x, y, w, h).contains(point) {
            self.modal_scroll -= self.wheel().1 * line_height * 3.;
        }
        if bar.max_offset > 0. {
            let hit = Rect::new(track.x - 7., y, 22., h);
            if is_mouse_button_pressed(MouseButton::Left) && hit.contains(point) {
                let in_thumb = (y + bar.thumb_offset..=y + bar.thumb_offset + bar.thumb_height)
                    .contains(&point.y);
                self.modal_scroll_grab = Some(if in_thumb {
                    point.y - y - bar.thumb_offset
                } else {
                    bar.thumb_height / 2.
                });
            }
            if let Some(grab) = self.modal_scroll_grab {
                self.modal_scroll = bar.drag_to(point.y - y, grab);
                self.click = None;
                self.dragged = true;
                if !is_mouse_button_down(MouseButton::Left) {
                    self.modal_scroll_grab = None;
                }
            } else if let Some((start, _)) = self.drag.clone()
                && content.contains(start)
                && is_mouse_button_down(MouseButton::Left)
                && (self.dragged || (start.y - point.y).abs() > 6.)
            {
                self.modal_scroll += start.y - point.y;
                self.drag = Some((point, None));
                self.dragged = true;
                self.click = None;
            }
        } else {
            self.modal_scroll_grab = None;
        }
        self.modal_scroll = self.modal_scroll.clamp(0., bar.max_offset);
        self.clip(content);
        let first = (self.modal_scroll / line_height).floor() as usize;
        for (i, line) in self.text_cache[&cache_key]
            .iter()
            .enumerate()
            .skip(first)
            .take((h / line_height).ceil() as usize + 1)
        {
            self.text(
                line,
                x,
                y + size + i as f32 * line_height - self.modal_scroll,
                size,
                INK,
            );
        }
        self.camera();
        if bar.max_offset > 0. {
            let bar = crate::interaction::Scrollbar::new(h, content_height, self.modal_scroll);
            self.panel(track, Color::new(0.88, 0.89, 0.85, 1.));
            self.panel(
                Rect::new(
                    track.x,
                    track.y + bar.thumb_offset,
                    track.w,
                    bar.thumb_height,
                ),
                MUTED,
            );
        }
    }
}
