use super::*;
impl App {
    pub(in crate::ui) fn play_screen(&mut self) {
        let (x, w) = self.content();
        let playing = self.game.outcome == Outcome::Playing;
        let idle = self.pending.is_none();
        self.card(Rect::new(x, 94., w, 84.));
        let board = self.game.board.clone();
        let state = if self.engine_failed {
            "引擎暂不可用"
        } else if let Some(p) = &self.pending {
            match p.job {
                Job::Move => "对方正在思考…",
                Job::Hint => "正在计算建议…",
                Job::End => "正在分析局势…",
                _ => "分析中…",
            }
        } else if !playing {
            "本局已结束"
        } else if board.in_check(board.turn) {
            "将军！请先应将"
        } else if self.selected.is_some() {
            "请选择绿色落点"
        } else if self.game.board.turn == self.player() {
            "轮到你走棋"
        } else {
            "轮到对方走棋"
        };
        let state_color = if self.engine_failed || board.in_check(board.turn) {
            RED_INK
        } else {
            JADE
        };
        draw_circle(x + 19., 120., 4., state_color);
        self.fit_text(state, x + 32., 127., w - 130., 21., state_color);
        let repeats = self.game.repetition_count();
        let save_status = self.saves.status(&self.game.id);
        let detail = if save_status == crate::session::SaveStatus::Failed {
            "保存失败 · 当前着法仍保留在内存中".into()
        } else if playing && repeats >= 2 {
            "重复局面预警：再次重复将判和或长将负".into()
        } else if playing && board.halfmove >= 100 {
            "自然限着预警：接近 60 回合无吃子".into()
        } else {
            format!(
                "你执{} · {}",
                if self.settings.user_red { "红" } else { "黑" },
                LEVELS[self.settings.level].name
            )
        };
        self.fit_text(
            &detail,
            x + 16.,
            158.,
            w - 32.,
            15.,
            if save_status == crate::session::SaveStatus::Failed {
                RED_INK
            } else {
                MUTED
            },
        );
        if self.engine_failed {
            if self.button("重试", Rect::new(x + w - 82., 104., 68., 40.), false, true) {
                self.engine_failed = false;
                self.notice.clear();
            }
        } else {
            self.fit_text(
                &format!("第 {} 回合", board.fullmove),
                x + w - 95.,
                125.,
                82.,
                15.,
                MUTED,
            );
        }
        self.board(
            &board,
            self.game.history.last().map(|e| e.mv),
            playing && idle && self.animation.is_none() && board.turn == self.player(),
        );
        if !self.landscape() {
            let (origin, cell) = self.board_rect();
            self.center(
                if self.settings.user_red {
                    "对手 · 执黑"
                } else {
                    "对手 · 执红"
                },
                self.width / 2.,
                origin.y - cell * 1.2 - 18.,
                15.,
                MUTED,
            );
            self.center(
                if self.settings.user_red {
                    "你 · 执红"
                } else {
                    "你 · 执黑"
                },
                self.width / 2.,
                origin.y + cell * 10.2 + 20.,
                15.,
                JADE,
            );
        }
        let strip_y = if self.landscape() {
            268.
        } else {
            self.height - 144.
        };
        let controls_y = self.height - 82.;
        let controls_h = 54.;
        if self.landscape() {
            self.text("本局棋谱", x + 2., strip_y - 16., 19., INK);
            if self.height >= 560. {
                self.card(Rect::new(x, 354., w, 106.));
                self.text("落子有声，行棋有据", x + 16., 383., 20., INK);
                self.fit_text(
                    "点选棋子，再点击绿色落点完成走棋。",
                    x + 16.,
                    413.,
                    w - 32.,
                    16.,
                    MUTED,
                );
                self.fit_text(
                    "提示展示两条建议；返回后可继续上局。",
                    x + 16.,
                    439.,
                    w - 32.,
                    16.,
                    MUTED,
                );
            }
        }
        let strip = Rect::new(x, strip_y, w, 44.);
        self.card(strip);
        if self.move_layout_key != (self.game.id.clone(), self.game.revision) {
            self.move_labels = self
                .game
                .history
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let text = format!("{}. {}", i + 1, e.notation);
                    let width = self.font.measure(&text, 17.).width;
                    (text, e.mover, width)
                })
                .collect();
            self.move_layout_key = (self.game.id.clone(), self.game.revision);
            self.move_offsets.clear();
            let mut offset = 0.;
            for (_, _, width) in &self.move_labels {
                self.move_offsets.push(offset);
                offset += width + 20.;
            }
            self.move_total = (offset - 20.).max(0.);
        }
        let total = self.move_total;
        let text_width = w - 24. - if self.move_follow { 0. } else { 76. };
        let max = (total - text_width).max(0.);
        let mouse = self.pointer();
        if self.modal.is_none() && strip.contains(mouse) {
            let wheel = self.wheel().1;
            if wheel != 0. {
                self.move_follow = false;
                self.move_scroll -= wheel * 45.;
            }
        }
        if self.modal.is_none()
            && is_mouse_button_down(MouseButton::Left)
            && let Some((start, _)) = self.drag.clone()
            && strip.contains(start)
            && (mouse.x - start.x).abs() > 8.
        {
            self.move_follow = false;
            self.move_scroll += start.x - mouse.x;
            self.drag = Some((mouse, None));
            self.dragged = true;
        }
        if self.move_follow {
            self.move_scroll = max;
        }
        self.move_scroll = self.move_scroll.clamp(0., max);
        self.clip(Rect::new(x + 10., strip_y + 1., text_width + 4., 42.));
        let start = self
            .move_offsets
            .partition_point(|offset| *offset < self.move_scroll)
            .saturating_sub(1);
        let mut tx =
            x + 12. - self.move_scroll + self.move_offsets.get(start).copied().unwrap_or(0.);
        for (text, side, width) in self.move_labels.iter().skip(start) {
            if tx >= x + 12. + text_width {
                break;
            }
            if tx + width >= x + 10. && tx < x + 12. + text_width {
                self.text(
                    text,
                    tx,
                    strip_y + 28.,
                    17.,
                    if *side == Side::Red { RED_INK } else { INK },
                );
            }
            tx += width + 20.;
        }
        self.camera();
        if self.game.history.is_empty() {
            self.center("棋谱随落子记录", x + w / 2., strip_y + 22., 15., MUTED);
        }
        if !self.move_follow
            && self.button(
                "最新",
                Rect::new(x + w - 68., strip_y + 5., 62., 34.),
                false,
                true,
            )
        {
            self.move_follow = true;
        }
        let bw = (w - 16.) / 3.;
        if self.button(
            "悔棋",
            Rect::new(x, controls_y, bw, controls_h),
            false,
            playing && !self.game.history.is_empty(),
        ) {
            self.cancel();
            self.animation = None;
            self.game.undo_to_player(self.player());
            self.engine_failed = false;
            self.persist();
        }
        if self.button(
            if self
                .pending
                .as_ref()
                .is_some_and(|p| matches!(p.job, Job::Hint))
            {
                "提示中…"
            } else {
                "提示"
            },
            Rect::new(x + bw + 8., controls_y, bw, controls_h),
            false,
            playing && idle && board.turn == self.player(),
        ) {
            self.search(Job::Hint);
        }
        if self.button(
            "结束对局",
            Rect::new(x + 2. * (bw + 8.), controls_y, bw, controls_h),
            false,
            playing,
        ) {
            self.cancel();
            self.open_modal(Modal::End(None));
            self.search(Job::End);
        }
    }
}
