use super::*;
impl App {
    pub(super) fn wide_difficulty(&mut self) {
        draw_rectangle(
            0.,
            0.,
            self.width,
            self.height,
            Color::new(0.12, 0.17, 0.15, 0.48),
        );
        let w = (self.width - 64.).min(940.);
        let x = (self.width - w) / 2.;
        let y = (self.height - 448.) / 2.;
        self.card(Rect::new(x, y, w, 448.));
        self.text("开启新对局", x + 28., y + 50., 28., INK);
        self.modal = Some(Modal::Difficulty);
        let left_w = (w - 84.) * 0.55;
        for (i, level) in LEVELS.iter().enumerate() {
            if self.button(
                level.name,
                Rect::new(x + 28., y + 82. + i as f32 * 66., left_w, 60.),
                false,
                true,
            ) {
                self.settings.level = i;
            }
            if self.settings.level == i {
                let row = Rect::new(x + 29., y + 83. + i as f32 * 66., left_w - 2., 58.);
                self.panel(row, Color::new(0.90, 0.94, 0.89, 1.));
                self.center(
                    level.name,
                    row.x + row.w / 2.,
                    row.y + row.h / 2.,
                    20.,
                    JADE,
                );
                draw_circle_lines(row.x + 24., row.y + row.h / 2., 7., 1.5, JADE);
                draw_circle(row.x + 24., row.y + row.h / 2., 3.5, JADE);
            }
        }
        let bx = x + left_w + 56.;
        let bw = w - left_w - 84.;
        self.fit_text(
            "简化裁决：长将负、重复和；未判长捉",
            bx,
            y + 112.,
            bw,
            16.,
            MUTED,
        );
        if self.button("执红开始", Rect::new(bx, y + 150., bw, 66.), true, true) {
            self.start_game(None, true);
        }
        if self.button("执黑开始", Rect::new(bx, y + 238., bw, 66.), false, true) {
            self.start_game(None, false);
        }
        if self.button("取消", Rect::new(bx, y + 344., bw, 58.), false, true) {
            self.modal = None;
        }
    }
    pub(super) fn modal_difficulty(&mut self, x: f32, y: f32, w: f32) {
        self.center("开启新对局", self.width / 2., y + 40., 28., INK);
        self.center(
            "选择难度与执子，即可开始",
            self.width / 2.,
            y + 76.,
            16.,
            MUTED,
        );
        self.modal = Some(Modal::Difficulty);
        for (i, l) in LEVELS.iter().enumerate() {
            let ry = y + 110. + i as f32 * 59.;
            self.semantic(
                format!(
                    "{}难度{}",
                    l.name,
                    if self.settings.level == i {
                        "，已选中"
                    } else {
                        ""
                    }
                ),
                Rect::new(x + 24., ry, w - 48., 50.),
                true,
            );
            let selected = self.settings.level == i;
            if self.button("", Rect::new(x + 24., ry, w - 48., 50.), false, true) {
                self.settings.level = i;
            }
            if selected {
                self.panel(
                    Rect::new(x + 25., ry + 1., w - 50., 48.),
                    Color::new(0.90, 0.94, 0.89, 1.),
                );
            }
            draw_circle_lines(
                x + 45.,
                ry + 25.,
                7.,
                1.5,
                if selected { JADE } else { BORDER },
            );
            if selected {
                draw_circle(x + 45., ry + 25., 3.5, JADE);
            }
            self.text(
                l.name,
                x + 65.,
                ry + 32.,
                20.,
                if selected { JADE } else { INK },
            );
            let descriptions = ["轻松上手", "熟悉棋路", "稳步进阶", "挑战棋力", "全力对弈"];
            self.text(descriptions[i], x + w - 124., ry + 30., 16., MUTED);
        }
        self.center(
            "简化裁决：长将负、重复和；未判长捉",
            self.width / 2.,
            y + 438.,
            15.,
            MUTED,
        );
        if self.button(
            "执红开始",
            Rect::new(x + 24., y + 467., (w - 60.) / 2., 54.),
            true,
            true,
        ) {
            self.start_game(None, true);
        }
        if self.button(
            "执黑开始",
            Rect::new(x + w / 2. + 6., y + 467., (w - 60.) / 2., 54.),
            false,
            true,
        ) {
            self.start_game(None, false);
        }
        if self.button(
            "取消",
            Rect::new(x + 24., y + 540., w - 48., 42.),
            false,
            true,
        ) {
            self.modal = None;
        }
    }
    pub(super) fn modal_side(&mut self, board: Option<Box<Game>>, x: f32, y: f32, w: f32) {
        self.center("选择执子", self.width / 2., y + 40., 28., INK);
        self.center(
            if board.is_some() {
                "保留此前棋谱，从当前局面继续"
            } else {
                "执红先行，执黑后手"
            },
            self.width / 2.,
            y + 86.,
            17.,
            MUTED,
        );
        let mut side = None;
        if self.button(
            "执红开始",
            Rect::new(x + 24., y + 137., w - 48., 54.),
            true,
            true,
        ) {
            side = Some(true);
        }
        if self.button(
            "执黑开始",
            Rect::new(x + 24., y + 207., w - 48., 54.),
            false,
            true,
        ) {
            side = Some(false);
        }
        if self.button(
            "取消",
            Rect::new(x + 24., y + 308., w - 48., 44.),
            false,
            true,
        ) {
            self.modal = None;
        } else if let Some(red) = side {
            self.start_game(board.map(|g| *g), red);
        } else {
            self.modal = Some(Modal::Side(board));
        }
    }
    pub(super) fn modal_end(
        &mut self,
        score: Option<crate::game::EvaluationEvidence>,
        x: f32,
        y: f32,
        w: f32,
    ) {
        self.center("结束此局", self.width / 2., y + 40., 28., INK);
        let text = score.as_ref().map_or_else(
            || {
                if self.engine_failed {
                    "引擎分析失败，可重试或直接认输".into()
                } else {
                    "正在分析当前局势…".into()
                }
            },
            |e| {
                format!(
                    "{} · {} · 深度 {}",
                    e.score.label(),
                    outcome_by_score(e.score.value()).label(),
                    e.reached_depth
                )
            },
        );
        self.fit_text(&text, x + 24., y + 106., w - 48., 18., MUTED);
        self.fit_text(
            "按局势结算会保存本局棋谱。",
            x + 24.,
            y + 143.,
            w - 48.,
            17.,
            INK,
        );
        self.modal = Some(Modal::End(score.clone()));
        if self.button(
            "认输并归档",
            Rect::new(x + 24., y + 200., w - 48., 48.),
            false,
            true,
        ) {
            self.finish_resign();
        }
        if self.button(
            "按当前局势结算",
            Rect::new(x + 24., y + 266., w - 48., 52.),
            true,
            score.is_some(),
        ) {
            self.game.settle_evaluated(score.clone().unwrap());
            self.modal = None;
            self.sound(if self.game.outcome == Outcome::Draw {
                5
            } else if self.game.outcome == Outcome::winner(self.player()) {
                3
            } else {
                4
            });
            self.persist();
        }
        if score.is_none()
            && self.engine_failed
            && self.button(
                "重新分析",
                Rect::new(x + 24., y + 333., w - 48., 40.),
                false,
                true,
            )
        {
            self.engine_failed = false;
            self.search(Job::End);
        }
        if self.button(
            "继续对弈",
            Rect::new(x + 24., y + 393., w - 48., 44.),
            false,
            true,
        ) {
            self.cancel();
            self.modal = None;
        }
    }
}
