use super::*;
impl App {
    pub(super) fn modal_variation(&mut self, text: String, x: f32, y: f32, w: f32) {
        self.center("参考变化", self.width / 2., y + 38., 26., INK);
        self.modal_text(&text, x + 24., y + 80., w - 48., 400., 18.);
        self.modal = Some(Modal::Variation(text));
        if self.button(
            "返回复盘",
            Rect::new(x + 24., y + 520., w - 48., 49.),
            true,
            true,
        ) {
            self.modal = None;
        }
    }
    pub(super) fn modal_help(&mut self, x: f32, y: f32, w: f32) {
        self.center("规则说明", self.width / 2., y + 38., 26., INK);
        let body = super::copy::HELP;
        self.modal_text(body, x + 22., y + 80., w - 44., 400., 18.);
        self.modal = Some(Modal::Help);
        if self.button(
            "我知道了",
            Rect::new(x + 24., y + 520., w - 48., 49.),
            true,
            true,
        ) {
            self.modal = None;
        }
    }
    pub(super) fn modal_about(&mut self, x: f32, y: f32, w: f32) {
        self.center("作者声明", self.width / 2., y + 36., 26., INK);
        let body = super::copy::ABOUT;
        self.modal_text(body, x + 22., y + 75., w - 44., 325., 17.);
        self.modal = Some(Modal::About);
        for (i, label) in ["GPLv3 全文", "NNUE 许可证", "字体 OFL 全文"]
            .iter()
            .enumerate()
        {
            if self.button(
                label,
                Rect::new(x + 24., y + 415. + i as f32 * 43., w - 48., 36.),
                false,
                true,
            ) {
                self.open_modal(Modal::License(i));
            }
        }
        if self.button(
            "确认",
            Rect::new(x + 24., y + 550., w - 48., 39.),
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
        self.modal_text(&body, x + 20., y + 75., w - 40., 425., 15.);
        self.modal = Some(Modal::License(index));
        if self.button(
            "返回声明",
            Rect::new(x + 24., y + 530., w - 48., 49.),
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
            self.modal_scroll -= mouse_wheel().1 * line_height * 3.;
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
