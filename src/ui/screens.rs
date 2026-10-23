use super::*;
impl App {
    pub(super) fn header(&mut self) {
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
    pub(super) fn home(&mut self) {
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
    pub(super) fn play_screen(&mut self) {
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
                "你执{} · {} · {}",
                if self.settings.user_red { "红" } else { "黑" },
                LEVELS[self.settings.level].name,
                if save_status == crate::session::SaveStatus::Pending {
                    "正在保存"
                } else {
                    "已保存"
                }
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
        if save_status == crate::session::SaveStatus::Failed {
            if self.button(
                "处理保存",
                Rect::new(x + w - 95., 104., 82., 40.),
                false,
                true,
            ) {
                self.open_modal(Modal::SaveFailed);
            }
        } else if self.engine_failed {
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
            let wheel = mouse_wheel().1;
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
    pub(super) fn archives(&mut self) {
        let (x, w) = self.content();
        if self.archive_dirty {
            self.archive_order = (0..self.data.archives.len())
                .filter(|&i| !self.archive_favorites || self.data.archives[i].favorite)
                .collect();
            self.archive_order.sort_by_key(|&i| {
                let r = &self.data.archives[i];
                (!r.favorite, std::cmp::Reverse(r.updated_at))
            });
            self.archive_dirty = false;
        }
        self.text("每一局，都值得回看", x + 2., 116., 24., INK);
        self.fit_text(
            &format!(
                "{} 局棋谱 · {} 局收藏",
                self.data.archives.len(),
                self.data.archives.iter().filter(|r| r.favorite).count()
            ),
            x + 2.,
            145.,
            w - 130.,
            15.,
            MUTED,
        );
        if self.button(
            "清空棋谱",
            Rect::new(x + w - 108., 119., 108., 38.),
            false,
            !self.data.archives.is_empty(),
        ) {
            self.open_modal(Modal::Clear);
        }
        if self.button(
            "全部棋谱",
            Rect::new(x, 176., (w - 10.) / 2., 42.),
            !self.archive_favorites,
            true,
        ) {
            self.archive_favorites = false;
            self.archive_dirty = true;
            self.scroll = 0.;
        }
        if self.button(
            "只看收藏",
            Rect::new(x + (w + 10.) / 2., 176., (w - 10.) / 2., 42.),
            self.archive_favorites,
            true,
        ) {
            self.archive_favorites = true;
            self.archive_dirty = true;
            self.scroll = 0.;
        }
        let viewport = Rect::new(x, 238., w, self.height - 272.);
        let order = self.archive_order.clone();
        let row_step = 124.;
        let max = (order.len() as f32 * row_step - 12. - viewport.h).max(0.);
        if self.modal.is_none() && viewport.contains(self.pointer()) {
            self.scroll -= mouse_wheel().1 * 46.;
        }
        self.scroll = self.scroll.clamp(0., max);
        if is_mouse_button_down(MouseButton::Left)
            && self.modal.is_none()
            && let Some((start, _)) = self.drag
        {
            let pos = self.pointer();
            if (pos.y - start.y).abs() > 10. && viewport.contains(start) {
                self.scroll = (self.scroll + start.y - pos.y).clamp(0., max);
                self.drag = Some((pos, None));
                self.dragged = true;
            }
        }
        // A camera viewport clips partially visible rows without discarding their content.
        self.clip(viewport);
        let outside_click = self.click.filter(|p| !viewport.contains(*p));
        self.click = self.click.filter(|p| viewport.contains(*p));
        for (i, idx) in order.into_iter().enumerate() {
            let y = viewport.y + i as f32 * row_step - self.scroll;
            if y + 112. < viewport.y || y > viewport.bottom() {
                continue;
            }
            let r = self.data.archives[idx].clone();
            self.card(Rect::new(x, y, w, 112.));
            let title = if r.title.is_empty() {
                format!(
                    "{}{}",
                    if r.mode == "review" { "续下 · " } else { "" },
                    date_label(r.time)
                )
            } else {
                r.title.clone()
            };
            let top = y.max(viewport.y);
            let bottom = (y + 112.).min(viewport.bottom());
            self.semantic(
                format!("复盘棋谱，{}，{}", title, r.result.label()),
                Rect::new(x, top, w - 150., (bottom - top).max(1.)),
                true,
            );
            self.fit_text(&title, x + 16., y + 29., w - 110., 19., INK);
            self.fit_text(
                &if r.title.is_empty() {
                    r.result.label().to_owned()
                } else {
                    format!("{} · {}", r.result.label(), date_label(r.time))
                },
                x + 16.,
                y + 59.,
                w - 32.,
                16.,
                if r.result == Outcome::Draw {
                    MUTED
                } else {
                    JADE
                },
            );
            self.fit_text(
                &format!(
                    "{} · {} · {} 步",
                    if r.user_red { "执红" } else { "执黑" },
                    r.level_name,
                    r.history.len()
                ),
                x + 16.,
                y + 88.,
                w - 158.,
                15.,
                MUTED,
            );
            if self.button(
                if r.favorite { "\u{2605}" } else { "\u{2606}" },
                Rect::new(x + w - 66., y + 10., 48., 42.),
                false,
                true,
            ) {
                self.enqueue(Command::Favorite(r.id.clone(), crate::game::new_id()));
            }
            if self.button(
                "命名",
                Rect::new(x + w - 140., y + 62., 56., 38.),
                false,
                true,
            ) {
                self.open_modal(Modal::Rename(r.id.clone(), r.title.clone().into()));
            }
            if self.button(
                "删除",
                Rect::new(x + w - 76., y + 62., 60., 38.),
                false,
                true,
            ) {
                self.open_modal(Modal::Delete(r.id.clone()));
            }
            if self
                .click
                .is_some_and(|p| Rect::new(x, y, w - 150., 112.).contains(p))
            {
                self.click = None;
                match r.replay() {
                    Ok(g) => {
                        self.settings.user_red = r.user_red;
                        self.open_review(g, Screen::Archives);
                        break;
                    }
                    Err(e) => self.notice = e,
                }
            }
            if let Some((start, _)) = self.drag.clone()
                && self.modal.is_none()
                && is_mouse_button_released(MouseButton::Left)
            {
                let end = self.pointer();
                if Rect::new(x, y, w, 112.).contains(start)
                    && end.x - start.x < -60.
                    && (end.y - start.y).abs() < 35.
                {
                    self.open_modal(Modal::Delete(r.id.clone()));
                }
            }
        }
        self.click = self.click.or(outside_click);
        self.camera();
        if self.archive_order.is_empty() {
            let cy = viewport.y + viewport.h * 0.40;
            self.card(Rect::new(x, cy - 94., w, 232.));
            self.center(
                if self.archive_favorites {
                    "还没有收藏的棋谱"
                } else {
                    "棋谱，等你落笔"
                },
                x + w / 2.,
                cy - 38.,
                25.,
                INK,
            );
            self.center(
                if self.archive_favorites {
                    "点击棋谱右侧的星标即可收藏"
                } else {
                    "完成对局后，棋谱会自动保存于此"
                },
                x + w / 2.,
                cy + 3.,
                16.,
                MUTED,
            );
            if self.button(
                if self.archive_favorites {
                    "查看全部棋谱"
                } else {
                    "开始一局"
                },
                Rect::new(x + 28., cy + 43., w - 56., 52.),
                true,
                true,
            ) {
                if self.archive_favorites {
                    self.archive_favorites = false;
                    self.archive_dirty = true;
                } else {
                    self.open_modal(Modal::Difficulty);
                }
            }
        }
        if max > 0. {
            let h = (viewport.h * viewport.h / (max + viewport.h)).max(24.);
            self.panel(
                Rect::new(
                    x + w - 4.,
                    viewport.y + self.scroll / max * (viewport.h - h),
                    3.,
                    h,
                ),
                GOLD,
            );
        }
    }
    pub(super) fn review_screen(&mut self) {
        let Some(r) = &self.review else {
            return;
        };
        let ply = r.ply;
        let n = r.game.history.len();
        let busy = r.analyzing;
        let board = r.game.at_ply(ply).unwrap().clone();
        let last = ply.checked_sub(1).map(|i| r.game.history[i].mv);
        self.board(&board, last, false);
        let (x, w) = self.content();
        let split = self.landscape();
        let card_y = if split { 100. } else { 544. };
        let card_h = if split && self.height < 560. {
            100.
        } else if split {
            124.
        } else {
            84.
        };
        self.card(Rect::new(x, card_y, w, card_h));
        let title = if ply == 0 {
            "初始局面".into()
        } else {
            format!(
                "{ply}. {}",
                self.review.as_ref().unwrap().game.history[ply - 1].notation
            )
        };
        self.fit_text(&title, x + 16., card_y + 31., w - 118., 22., INK);
        if self.engine_failed {
            if self.button(
                "重试分析",
                Rect::new(x + w - 99., card_y + 10., 86., 37.),
                false,
                true,
            ) {
                self.engine_failed = false;
                self.notice.clear();
                self.review.as_mut().unwrap().analyzing = true;
            }
        } else {
            self.fit_text(
                &format!("{ply} / {n} 步"),
                x + w - 95.,
                card_y + 29.,
                80.,
                15.,
                MUTED,
            );
        }
        let r = self.review.as_ref().unwrap();
        let mut detail = "点击走势可跳转到任意一步".to_owned();
        if ply > 0 {
            let e = &r.game.history[ply - 1];
            let quality = if r.bests[ply - 1] == Some(e.mv) {
                Some("优秀")
            } else {
                r.scores[ply - 1]
                    .zip(r.scores[ply])
                    .map(|(a, b)| crate::engine::assess_move(a, b, e.mover).label())
            };
            detail = quality.map_or_else(
                || "着法评价将在分析后显示".into(),
                |q| format!("按当前分析：{q}"),
            );
            if let Some(m) = r.bests[ply - 1] {
                if m == e.mv {
                    detail.push_str(" · 最佳走法");
                } else {
                    detail.push_str(&format!(
                        " · 建议 {}",
                        crate::notation::chinese(r.game.at_ply(ply - 1).unwrap(), m)
                            .unwrap_or_else(|_| m.to_string())
                    ));
                }
            }
        }
        self.fit_text(&detail, x + 16., card_y + 64., w - 32., 16., JADE);
        if split && self.height >= 560. {
            self.fit_text(
                "随时翻页，后台分析不影响当前局面",
                x + 16.,
                card_y + 99.,
                w - 32.,
                15.,
                MUTED,
            );
        }
        let r = self.review.as_ref().unwrap();
        let done = r.scores.iter().filter(|s| s.is_some()).count();
        let score = r.scores[ply].map_or_else(|| "局势待分析".into(), Score::label);
        let progress_y = if split && self.height < 560. {
            220.
        } else if split {
            263.
        } else {
            663.
        };
        self.panel(Rect::new(x, progress_y, w, 4.), BORDER);
        if done > 0 {
            self.panel(
                Rect::new(x, progress_y, w * done as f32 / (n + 1) as f32, 4.),
                JADE,
            );
        }
        let analysis_state = if self.engine_failed {
            "分析失败"
        } else if done == n + 1 {
            "分析完成"
        } else if busy {
            "正在分析"
        } else {
            "已暂停"
        };
        self.fit_text(
            &format!("{analysis_state} · {done}/{}", n + 1),
            x + 2.,
            progress_y - 9.,
            w * 0.54,
            14.,
            MUTED,
        );
        let status = if r.scores[ply].is_some() {
            format!("{score} · 深度 {}", r.depths[ply])
        } else {
            score
        };
        self.fit_text(
            &status,
            x + w * 0.54,
            progress_y - 9.,
            w * 0.46 - 2.,
            14.,
            MUTED,
        );
        let chart_y = if split && self.height < 560. {
            240.
        } else if split {
            284.
        } else {
            683.
        };
        let chart_bottom = if split {
            self.height - 170.
        } else {
            self.height - 108.
        };
        self.draw_review_chart(Rect::new(x, chart_y, w, chart_bottom - chart_y));
        let mode_y = if split { self.height - 156. } else { 96. };
        if self.button(
            if done == n + 1 {
                "分析完成"
            } else if busy {
                "暂停分析"
            } else {
                "继续分析"
            },
            Rect::new(x, mode_y, w, 40.),
            false,
            done < n + 1,
        ) {
            if busy {
                self.save_review_cache();
                self.engine.cancel();
                self.pending = None;
                self.review.as_mut().unwrap().analyzing = false;
            } else {
                self.engine_failed = false;
                self.review.as_mut().unwrap().analyzing = true;
            }
        }
        let controls_y = self.height - 82.;
        let bw = (w - 24.) / 4.;
        let mut target = ply;
        for (i, label, dest, enabled) in [
            (0, "首步", 0, ply > 0),
            (1, "上一步", ply.saturating_sub(1), ply > 0),
            (2, "下一步", (ply + 1).min(n), ply < n),
            (3, "末步", n, ply < n),
        ] {
            if self.button(
                label,
                Rect::new(x + i as f32 * (bw + 8.), controls_y, bw, 54.),
                false,
                enabled,
            ) {
                target = dest;
            }
        }
        if self.modal.is_none() {
            if is_key_pressed(KeyCode::Left) {
                target = ply.saturating_sub(1);
            }
            if is_key_pressed(KeyCode::Right) {
                target = (ply + 1).min(n);
            }
            if is_key_pressed(KeyCode::Home) {
                target = 0;
            }
            if is_key_pressed(KeyCode::End) {
                target = n;
            }
        }
        // The chart may already have changed the cursor this frame.
        if target != ply {
            self.review.as_mut().unwrap().ply = target;
        }
    }
    pub(super) fn draw_review_chart(&mut self, chart: Rect) {
        self.card(chart);
        self.text("局势走势", chart.x + 12., chart.y + 23., 15., INK);
        self.fit_text(
            "红优 / 黑优",
            chart.x + chart.w - 102.,
            chart.y + 23.,
            92.,
            13.,
            MUTED,
        );
        let plot = Rect::new(
            chart.x + 14.,
            chart.y + 34.,
            chart.w - 28.,
            (chart.h - 46.).max(20.),
        );
        let r = self.review.as_mut().unwrap();
        let columns = plot.w.ceil().max(1.) as usize;
        if r.chart_cache.0 != r.score_version || r.chart_cache.1 != columns {
            r.chart_cache = (
                r.score_version,
                columns,
                crate::review::chart_samples(&r.scores, columns),
            );
        }
        let r = self.review.as_ref().unwrap();
        let n = r.game.history.len();
        let middle = plot.y + plot.h / 2.;
        for fraction in [0.25, 0.5, 0.75] {
            let y = plot.y + plot.h * fraction;
            draw_line(
                plot.x,
                y,
                plot.x + plot.w,
                y,
                1.,
                if fraction == 0.5 { BORDER } else { PAPER },
            );
        }
        let point = |i: usize, cp: i32| {
            vec2(
                plot.x + i as f32 / n.max(1) as f32 * plot.w,
                middle - (cp as f32 / 600.).tanh() * plot.h * 0.44,
            )
        };
        for pair in r.chart_cache.2.windows(2) {
            if let (Some(a), Some(b)) = (pair[0].1, pair[1].1) {
                let a = point(pair[0].0, a.value());
                let b = point(pair[1].0, b.value());
                draw_triangle(
                    vec2(a.x, middle),
                    a,
                    b,
                    Color::new(JADE.r, JADE.g, JADE.b, 0.09),
                );
                draw_triangle(
                    vec2(a.x, middle),
                    b,
                    vec2(b.x, middle),
                    Color::new(JADE.r, JADE.g, JADE.b, 0.09),
                );
                draw_line(a.x, a.y, b.x, b.y, 2.5, JADE);
            }
        }
        let cursor = plot.x + r.ply as f32 / n.max(1) as f32 * plot.w;
        draw_line(cursor, plot.y, cursor, plot.y + plot.h, 1.5, GOLD);
        if let Some(score) = r.scores[r.ply] {
            let p = point(r.ply, score.value());
            draw_circle(p.x, p.y, 5., SURFACE);
            draw_circle(p.x, p.y, 3.5, JADE);
        }
        if r.scores.iter().all(Option::is_none) {
            self.center(
                "分析后显示局势变化",
                plot.x + plot.w / 2.,
                middle,
                14.,
                MUTED,
            );
        }
        let pointer = self.pointer();
        let dragging = self.modal.is_none()
            && is_mouse_button_down(MouseButton::Left)
            && self
                .drag
                .as_ref()
                .is_some_and(|(start, _)| chart.contains(*start));
        let selected = self.click.filter(|p| chart.contains(*p)).or(if dragging {
            Some(pointer)
        } else {
            None
        });
        if let Some(p) = selected {
            self.review.as_mut().unwrap().ply =
                (((p.x - plot.x) / plot.w).clamp(0., 1.) * n as f32).round() as usize;
            self.click = None;
        }
    }
    pub(super) fn end_overlay(&mut self) {
        if !self.end_ready
            && matches!(
                self.game.end_reason,
                Some(crate::game::EndReason::Checkmate)
            )
        {
            let ended_at = *self.ended_at.get_or_insert_with(get_time);
            if get_time() - ended_at < 1. {
                return;
            }
        }
        self.end_ready = true;
        draw_rectangle(
            0.,
            0.,
            self.width,
            self.height,
            Color::new(0.12, 0.17, 0.15, 0.55),
        );
        let w = (self.width - 40.).min(460.);
        let h = 456.;
        let x = (self.width - w) / 2.;
        let y = (self.height - h) / 2.;
        self.card(Rect::new(x, y, w, h));
        let title = if self.game.outcome == Outcome::Draw {
            "握手言和"
        } else if self.game.outcome == Outcome::winner(self.player()) {
            "此局获胜"
        } else {
            "胜败之间，皆有所得"
        };
        self.center(
            title,
            self.width / 2.,
            y + 55.,
            30.,
            if self.game.outcome == Outcome::winner(self.player()) {
                JADE
            } else {
                INK
            },
        );
        self.fit_text(&self.game.reason, x + 24., y + 102., w - 48., 17., MUTED);
        self.panel(Rect::new(x + 24., y + 129., w - 48., 60.), PAPER);
        let saved = if !self.saves.failed.is_empty() {
            "等待重试保存"
        } else if self.saves.pending > 0 {
            "正在保存棋谱"
        } else {
            "已自动归档"
        };
        self.center(
            &format!(
                "{} 步 · {} · {saved}",
                self.game.history.len(),
                LEVELS[self.settings.level].name
            ),
            self.width / 2.,
            y + 159.,
            17.,
            JADE,
        );
        if self.button(
            "复盘此局",
            Rect::new(x + 24., y + 216., w - 48., 54.),
            true,
            !self.game.history.is_empty(),
        ) {
            self.open_review(self.game.clone(), Screen::Game);
        }
        if !self.review_game
            && self.button(
                "再来一局",
                Rect::new(x + 24., y + 287., w - 48., 50.),
                false,
                true,
            )
        {
            self.start_game(None, self.settings.user_red);
        }
        if self.button(
            if self.review_game {
                "返回复盘"
            } else {
                "返回首页"
            },
            Rect::new(x + 24., y + 358., w - 48., 50.),
            false,
            true,
        ) {
            self.back();
        }
    }
}
pub(super) fn date_label(ms: u64) -> String {
    let s = ms / 1000 + 8 * 3600;
    let z = (s / 86400) as i64 + 719468;
    let era = z / 146097;
    let d = z - era * 146097;
    let y = (d - d / 1460 + d / 36524 - d / 146096) / 365;
    let mut year = y + era * 400;
    let day = d - (365 * y + y / 4 - y / 100);
    let m = (5 * day + 2) / 153;
    let dd = day - (153 * m + 2) / 5 + 1;
    let mm = m + if m < 10 { 3 } else { -9 };
    if mm <= 2 {
        year += 1;
    }
    format!(
        "{year:04}-{mm:02}-{dd:02} {:02}:{:02}",
        s / 3600 % 24,
        s / 60 % 60
    )
}
