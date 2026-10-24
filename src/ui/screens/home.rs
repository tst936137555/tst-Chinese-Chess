use super::*;
impl App {
    pub(in crate::ui) fn header(&mut self) {
        draw_rectangle(0., 0., self.width, 77., SURFACE);
        draw_line(0., 77., self.width, 77., 1., BORDER);
        let hw = self.width.min(1240.);
        let left = (self.width - hw) / 2.;
        let right = left + hw;
        let title = match self.screen {
            Screen::Home => "中国象棋",
            Screen::Game => "对弈",
            Screen::Archives => "棋谱收藏",
            Screen::Review => "复盘",
        };
        if self.screen == Screen::Home {
            self.panel(Rect::new(left + 20., 22., 32., 34.), RED_INK);
            self.center("象", left + 36., 38., 23., SURFACE);
        } else if self.button("返回", Rect::new(left + 16., 18., 62., 42.), false, true) {
            self.back();
            return;
        }
        self.text(
            title,
            left + if self.screen == Screen::Home {
                64.
            } else {
                94.
            },
            48.,
            25.,
            INK,
        );
        match self.screen {
            Screen::Home => {
                if self.button("声音", Rect::new(right - 146., 18., 62., 42.), false, true) {
                    self.open_modal(Modal::Audio);
                }
                if self.button("关于", Rect::new(right - 76., 18., 60., 42.), false, true) {
                    self.open_modal(Modal::About);
                }
            }
            Screen::Game => {
                let busy = self.pending.is_some() || self.game.outcome != Outcome::Playing;
                if self.button(
                    "-",
                    Rect::new(right - 162., 18., 40., 42.),
                    false,
                    !busy && self.settings.level > 0,
                ) {
                    self.change_level(-1);
                }
                self.center(
                    LEVELS[self.settings.level].name,
                    right - 90.,
                    39.,
                    18.,
                    JADE,
                );
                if self.button(
                    "+",
                    Rect::new(right - 56., 18., 40., 42.),
                    false,
                    !busy && self.settings.level < 4,
                ) {
                    self.change_level(1);
                }
            }
            Screen::Archives => {
                if self.button("说明", Rect::new(right - 80., 18., 64., 42.), false, true) {
                    self.open_modal(Modal::Help);
                }
            }
            Screen::Review => {
                let can_continue = self
                    .review
                    .as_ref()
                    .is_some_and(|r| r.game.can_branch_at(r.ply));
                if self.button(
                    "此处续下",
                    Rect::new(right - 174., 18., 108., 42.),
                    false,
                    can_continue,
                ) {
                    let r = self.review.as_ref().unwrap();
                    if let Ok(b) = r.game.branch_at(r.ply) {
                        self.open_modal(Modal::Side(Some(Box::new(b))));
                    }
                }
                if self.button("?", Rect::new(right - 58., 18., 42., 42.), false, true) {
                    self.open_modal(Modal::Help);
                }
            }
        }
    }
    pub(in crate::ui) fn home(&mut self) {
        let wide = self.width >= 920.;
        let w = (self.width - 40.).min(if wide { 980. } else { 500. });
        let h = if wide { 580. } else { 670. };
        let x = (self.width - w) / 2.;
        let y = 100. + (self.height - 130. - h).max(0.) / 2.;
        self.card(Rect::new(x, y, w, h));
        let hero_x = if wide { x + w * 0.25 } else { self.width / 2. };
        let hero_y = y + if wide { 125. } else { 70. };
        draw_circle(hero_x, hero_y + 3., 40., Color::new(0.22, 0.42, 0.35, 0.12));
        draw_circle(hero_x, hero_y, 39., JADE);
        draw_circle_lines(hero_x, hero_y, 32., 1., Color::new(1., 1., 1., 0.5));
        self.center("棋", hero_x, hero_y - 1., 43., SURFACE);
        self.center(
            "方寸之间，自有天地",
            hero_x,
            hero_y + 78.,
            if wide { 29. } else { 27. },
            INK,
        );
        self.center("静心对弈 · 步步有思", hero_x, hero_y + 116., 17., MUTED);
        if wide {
            draw_line(x + w * 0.49, y + 48., x + w * 0.49, y + h - 48., 1., BORDER);
            self.home_motif(vec2(hero_x - 128., y + 325.), 32.);
        }
        let bx = if wide { x + w * 0.55 } else { x + 24. };
        let bw = if wide { w * 0.39 } else { w - 48. };
        let by = y + if wide { 112. } else { 242. };
        let has_save = self.data.active.is_some() || !self.data.drafts.is_empty();
        if wide {
            self.text("开启一局好棋", bx, by - 31., 26., INK);
        }
        if self.button("开始新对局", Rect::new(bx, by, bw, 58.), true, true) {
            self.open_modal(Modal::Difficulty);
        }
        if self.button(
            if has_save {
                "继续未完成对局"
            } else {
                "暂无未完成对局"
            },
            Rect::new(bx, by + 74., bw, 54.),
            false,
            has_save,
        ) {
            self.resume();
        }
        if self.button(
            "浏览与复盘棋谱",
            Rect::new(bx, by + 144., bw, 54.),
            false,
            true,
        ) {
            self.reload();
            self.scroll = 0.;
            self.screen = Screen::Archives;
        }
        let sy = by + 235.;
        self.panel(Rect::new(bx, sy, bw, 68.), Color::new(0.93, 0.95, 0.91, 1.));
        self.center(
            &self.data.archives.len().to_string(),
            bx + bw * 0.25,
            sy + 23.,
            24.,
            JADE,
        );
        self.center("已存棋谱", bx + bw * 0.25, sy + 49., 14., MUTED);
        draw_line(bx + bw / 2., sy + 16., bx + bw / 2., sy + 52., 1., BORDER);
        self.center("五档", bx + bw * 0.75, sy + 23., 24., JADE);
        self.center("对弈难度", bx + bw * 0.75, sy + 49., 14., MUTED);
        let fy = y + h - 64.;
        if self.button("玩法说明", Rect::new(bx, fy, bw, 42.), false, true) {
            self.open_modal(Modal::Help);
        }
        self.center(
            concat!("tst 自用象棋 · 离线对弈 · v", env!("CARGO_PKG_VERSION")),
            self.width / 2.,
            y + h + 34.,
            14.,
            MUTED,
        );
    }
    fn home_motif(&self, origin: Vec2, cell: f32) {
        let r = Rect::new(
            origin.x - 18.,
            origin.y - 18.,
            cell * 8. + 36.,
            cell * 3. + 36.,
        );
        self.panel(r, Color::new(0.94, 0.90, 0.81, 1.));
        for rank in 0..4 {
            draw_line(
                origin.x,
                origin.y + rank as f32 * cell,
                origin.x + cell * 8.,
                origin.y + rank as f32 * cell,
                1.,
                BORDER,
            );
        }
        for file in 0..9 {
            draw_line(
                origin.x + file as f32 * cell,
                origin.y,
                origin.x + file as f32 * cell,
                origin.y + cell * 3.,
                1.,
                BORDER,
            );
        }
        for (file, rank, label, red) in [
            (1., 1., "马", false),
            (4., 2., "炮", true),
            (6., 0., "象", false),
            (7., 3., "兵", true),
        ] {
            let p = origin + vec2(file * cell, rank * cell);
            draw_circle(p.x, p.y + 2., cell * 0.43, BORDER);
            draw_circle(p.x, p.y, cell * 0.43, SURFACE);
            draw_circle_lines(p.x, p.y, cell * 0.35, 1., if red { RED_INK } else { JADE });
            self.center(
                label,
                p.x,
                p.y,
                cell * 0.56,
                if red { RED_INK } else { JADE },
            );
        }
    }
}
