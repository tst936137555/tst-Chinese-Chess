use super::*;
impl App {
    pub(super) fn draw_modal(&mut self) {
        let Some(modal) = self.modal.take() else {
            return;
        };
        if self.landscape() && matches!(modal, Modal::Audio) {
            self.wide_audio();
            return;
        }
        let old_canvas = (self.width, self.height);
        let compact_name = matches!(modal, Modal::Rename(..)) && self.viewport.h < 380.;
        let required_height: f32 = match &modal {
            Modal::Side(_) => 460.,
            Modal::End(_) => 540.,
            Modal::SaveFailed => 500.,
            Modal::StorageRecovery => 500.,
            Modal::Rename(..) => {
                if compact_name {
                    240.
                } else {
                    340.
                }
            }
            Modal::Delete(_) | Modal::Clear | Modal::ClearDrafts => 380.,
            _ => 685.,
        };
        let scale = self.height.min(self.viewport.h.max(required_height)) / self.height;
        if scale != 1. {
            self.width *= scale;
            self.height *= scale;
            self.click = self.click.map(|p| p * scale);
            if let Some((p, id)) = self.drag.take() {
                self.drag = Some((p * scale, id));
            }
            self.camera();
        }
        draw_rectangle(
            0.,
            0.,
            self.width,
            self.height,
            Color::new(0.12, 0.17, 0.15, 0.48),
        );
        let w = (self.width - 40.).min(480.);
        let x = (self.width - w) / 2.;
        let modal_height = match &modal {
            Modal::Side(_) => 380.,
            Modal::End(_) => 460.,
            Modal::SaveFailed => 420.,
            Modal::StorageRecovery => 420.,
            Modal::Rename(..) => {
                if compact_name {
                    212.
                } else {
                    292.
                }
            }
            Modal::Delete(_) | Modal::Clear | Modal::ClearDrafts => 300.,
            Modal::Drafts(_) => {
                let count = self.data.active.iter().count() + self.data.drafts.len();
                if count == 0 { 300. } else { 605. }
            }
            _ => 605.,
        };
        let y = (self.height - modal_height) / 2.;
        self.card(Rect::new(x, y, w, modal_height));
        match modal {
            Modal::ClearDrafts => {
                self.center("删除全部旧对局？", self.width / 2., y + 44., 25., INK);
                self.center("保留当前活动对局", self.width / 2., y + 103., 17., MUTED);
                self.modal = Some(Modal::ClearDrafts);
                if self.button(
                    "确认删除旧对局",
                    Rect::new(x + 24., y + 160., w - 48., 48.),
                    true,
                    true,
                ) {
                    self.enqueue(Command::ClearDrafts);
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
            Modal::Rename(id, mut edit) => {
                let archived = self.data.archives.iter().any(|r| r.id == id);
                self.center(
                    if archived {
                        "棋谱名称"
                    } else {
                        "对局名称"
                    },
                    self.width / 2.,
                    y + 38.,
                    26.,
                    INK,
                );
                self.keyboard_active = !self.name_keyboard;
                let ctrl = is_key_down(KeyCode::LeftControl)
                    || is_key_down(KeyCode::RightControl)
                    || is_key_down(KeyCode::LeftSuper)
                    || is_key_down(KeyCode::RightSuper);
                let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                if self.name_keyboard {
                    if ctrl && is_key_pressed(KeyCode::A) {
                        edit.select_all();
                    }
                    if ctrl && is_key_pressed(KeyCode::C) {
                        macroquad::miniquad::window::clipboard_set(&edit.selected());
                    }
                    if ctrl && is_key_pressed(KeyCode::X) {
                        macroquad::miniquad::window::clipboard_set(&edit.selected());
                        edit.insert("");
                    }
                    if ctrl
                        && is_key_pressed(KeyCode::V)
                        && let Some(value) = macroquad::miniquad::window::clipboard_get()
                    {
                        edit.insert(&value);
                    }
                    let mut deleted_back = false;
                    let mut deleted_forward = false;
                    while let Some(ch) = get_char_pressed() {
                        if !ctrl {
                            match ch {
                                '\u{8}' => {
                                    edit.backspace();
                                    deleted_back = true;
                                }
                                '\u{7f}' => {
                                    edit.delete();
                                    deleted_forward = true;
                                }
                                c if !c.is_control() => edit.insert(&c.to_string()),
                                _ => {}
                            }
                        }
                    }
                    if !deleted_back && is_key_pressed(KeyCode::Backspace) {
                        edit.backspace();
                    }
                    if !deleted_forward && is_key_pressed(KeyCode::Delete) {
                        edit.delete();
                    }
                    if is_key_pressed(KeyCode::Left) {
                        edit.move_to(edit.cursor.saturating_sub(1), shift);
                    }
                    if is_key_pressed(KeyCode::Right) {
                        edit.move_to(edit.cursor + 1, shift);
                    }
                    if is_key_pressed(KeyCode::Home) {
                        edit.move_to(0, shift);
                    }
                    if is_key_pressed(KeyCode::End) {
                        edit.move_to(edit.text.chars().count(), shift);
                    }
                    if is_key_pressed(KeyCode::Tab) {
                        self.name_keyboard = false;
                        macroquad::miniquad::window::show_keyboard(false);
                    }
                }
                let field_y = if compact_name { 60. } else { 72. };
                let actions_y = if compact_name { 146. } else { 218. };
                let field = Rect::new(x + 24., y + field_y, w - 116., 54.);
                self.panel(field, PAPER);
                let chars: Vec<_> = edit.text.chars().collect();
                let prefix = |a: usize, b: usize| chars[a..b].iter().collect::<String>();
                let text_width = field.w - 24.;
                let mut start = 0;
                while start < edit.cursor
                    && self.font.measure(&prefix(start, edit.cursor), 20.).width > text_width
                {
                    start += 1;
                }
                let mut end = edit.cursor;
                while end < chars.len()
                    && self.font.measure(&prefix(start, end + 1), 20.).width <= text_width
                {
                    end += 1;
                }
                let text_x = field.x + 12.;
                if self.click.is_some_and(|p| field.contains(p)) {
                    let click = self.click.take().unwrap();
                    let cursor = (start..=end)
                        .min_by(|&a, &b| {
                            (self.font.measure(&prefix(start, a), 20.).width - (click.x - text_x))
                                .abs()
                                .total_cmp(
                                    &(self.font.measure(&prefix(start, b), 20.).width
                                        - (click.x - text_x))
                                        .abs(),
                                )
                        })
                        .unwrap_or(end);
                    edit.move_to(cursor, shift);
                    self.name_keyboard = true;
                    macroquad::miniquad::window::show_keyboard(true);
                }
                let selection = edit.selection();
                let a = selection.start.max(start).min(end);
                let b = selection.end.max(start).min(end);
                if a < b {
                    let left = self.font.measure(&prefix(start, a), 20.).width;
                    let right = self.font.measure(&prefix(start, b), 20.).width;
                    draw_rectangle(
                        text_x + left,
                        field.y + 12.,
                        right - left,
                        30.,
                        Color::new(0.79, 0.88, 0.81, 1.),
                    );
                }
                self.text(&prefix(start, end), text_x, field.y + 34., 20., INK);
                if self.name_keyboard && ((get_time() * 2.) as u64).is_multiple_of(2) {
                    let cursor_x =
                        text_x + self.font.measure(&prefix(start, edit.cursor), 20.).width;
                    draw_line(cursor_x, field.y + 12., cursor_x, field.y + 40., 1.5, JADE);
                }
                if self.button(
                    "粘贴",
                    Rect::new(x + w - 84., y + field_y, 60., 54.),
                    false,
                    true,
                ) && let Some(value) = macroquad::miniquad::window::clipboard_get()
                {
                    edit.insert(&value);
                }
                for (i, label) in ["开局练习", "残局练习", "待续对局"].iter().enumerate()
                {
                    if compact_name {
                        break;
                    }
                    if self.button(
                        label,
                        Rect::new(
                            x + 24. + i as f32 * (w - 40.) / 3.,
                            y + 148.,
                            (w - 64.) / 3.,
                            42.,
                        ),
                        false,
                        true,
                    ) {
                        edit = (*label).into();
                    }
                }
                self.modal = Some(Modal::Rename(id.clone(), edit.clone()));
                if self.button(
                    "保存名称",
                    Rect::new(x + w / 2. + 4., y + actions_y, (w - 56.) / 2., 48.),
                    true,
                    true,
                ) || (self.name_keyboard && is_key_pressed(KeyCode::Enter))
                {
                    self.enqueue(Command::Rename(id, edit.text));
                    self.finish_rename();
                }
                if self.button(
                    "取消",
                    Rect::new(x + 24., y + actions_y, (w - 56.) / 2., 48.),
                    false,
                    true,
                ) {
                    self.finish_rename();
                }
            }
            Modal::Audio => {
                self.center("声音设置", self.width / 2., y + 40., 26., INK);
                self.modal = Some(Modal::Audio);
                let mut changed = false;
                if self.button(
                    if self.settings.sound {
                        "声音：开启"
                    } else {
                        "声音：静音"
                    },
                    Rect::new(x + 24., y + 85., w - 48., 47.),
                    false,
                    true,
                ) {
                    self.settings.sound = !self.settings.sound;
                    changed = true;
                }
                for (i, label) in ["背景音乐", "棋子音效"].iter().enumerate() {
                    let enabled = if i == 0 {
                        self.settings.music_enabled
                    } else {
                        self.settings.effects_enabled
                    };
                    let volume = if i == 0 {
                        self.settings.music_volume
                    } else {
                        self.settings.effects_volume
                    };
                    let row = y + 170. + i as f32 * 143.;
                    let audible = self.settings.sound && enabled;
                    if self.button(
                        &format!("{label}：{}", if enabled { "开" } else { "关" }),
                        Rect::new(x + 24., row, w - 48., 42.),
                        false,
                        true,
                    ) {
                        if i == 0 {
                            self.settings.music_enabled = !enabled;
                        } else {
                            self.settings.effects_enabled = !enabled;
                        }
                        changed = true;
                    }
                    self.center(
                        &format!("{}%", (volume * 100.).round() as u32),
                        self.width / 2.,
                        row + 82.,
                        21.,
                        INK,
                    );
                    if self.button(
                        "-",
                        Rect::new(x + 24., row + 60., 70., 42.),
                        false,
                        volume > 0.,
                    ) {
                        if i == 0 {
                            self.settings.music_volume = (volume - 0.1).max(0.);
                        } else {
                            self.settings.effects_volume = (volume - 0.1).max(0.);
                        }
                        changed = true;
                    }
                    if self.button(
                        "+",
                        Rect::new(x + w - 94., row + 60., 70., 42.),
                        false,
                        volume < 1.,
                    ) {
                        if i == 0 {
                            self.settings.music_volume = (volume + 0.1).min(1.);
                        } else {
                            self.settings.effects_volume = (volume + 0.1).min(1.);
                        }
                        changed = true;
                    }
                    let track_x = x + 108.;
                    let track_w = w - 216.;
                    let track = Rect::new(track_x - 8., row + 87., track_w + 16., 36.);
                    let pointer = self.pointer();
                    let dragging = is_mouse_button_down(MouseButton::Left)
                        && self
                            .drag
                            .as_ref()
                            .is_some_and(|(start, _)| track.contains(*start));
                    if self.click.is_some_and(|p| track.contains(p)) || dragging {
                        let pointer = self.click.unwrap_or(pointer);
                        let value = ((pointer.x - track_x) / track_w).clamp(0., 1.);
                        if i == 0 {
                            self.settings.music_volume = value;
                        } else {
                            self.settings.effects_volume = value;
                        }
                        self.click = None;
                        // Persist once at release; changing the in-memory setting previews immediately.
                        changed |= is_mouse_button_released(MouseButton::Left);
                    }
                    draw_line(
                        track_x,
                        row + 105.,
                        track_x + track_w,
                        row + 105.,
                        3.,
                        Color::new(0.80, 0.74, 0.63, 1.),
                    );
                    draw_line(
                        track_x,
                        row + 105.,
                        track_x + track_w * volume,
                        row + 105.,
                        3.,
                        if audible { RED_INK } else { GOLD },
                    );
                    draw_circle(
                        track_x + track_w * volume,
                        row + 105.,
                        4.,
                        if audible { RED_INK } else { GOLD },
                    );
                }
                self.center(
                    if self.settings.sound {
                        "立体声 · 落点定位 · 提示时音乐轻降"
                    } else {
                        "已静音，音量偏好仍会保留"
                    },
                    self.width / 2.,
                    y + 490.,
                    14.,
                    MUTED,
                );
                if is_mouse_button_released(MouseButton::Left) && self.dragged {
                    changed = true;
                }
                if changed {
                    self.enqueue(Command::Settings(self.settings.clone()));
                }
                if self.button(
                    "确认",
                    Rect::new(x + 24., y + 535., w - 48., 48.),
                    true,
                    true,
                ) {
                    self.modal = None;
                }
            }
            Modal::SaveFailed => {
                self.center("对局尚未保存", self.width / 2., y + 40., 26., INK);
                self.modal = Some(Modal::SaveFailed);
                self.modal_text(
                    "自动保存失败，当前棋盘已保留。\n可重试，或另存当前内容后退出。",
                    x + 24.,
                    y + 75.,
                    w - 48.,
                    60.,
                    17.,
                );
                if self.button(
                    "重试保存",
                    Rect::new(x + 24., y + 145., w - 48., 48.),
                    true,
                    !self.saves.failed.is_empty(),
                ) {
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
                    self.closing = true;
                    self.close_flushed = false;
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
                    self.force_exit = true;
                }
            }
            Modal::StorageRecovery => {
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
            Modal::Drafts(page) => {
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
                    let pages = records.len().max(1).div_ceil(5);
                    let page = page.min(pages - 1);
                    self.modal = Some(Modal::Drafts(page));
                    for (i, r) in records.iter().skip(page * 5).take(5).enumerate() {
                        let label = format!(
                            "{} · {} · {}步",
                            if r.user_red { "执红" } else { "执黑" },
                            r.level_name,
                            r.history.len()
                        );
                        if self.button(
                            &label,
                            Rect::new(x + 24., y + 80. + i as f32 * 73., w - 160., 40.),
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
                            y + 139. + i as f32 * 73.,
                            w - 56.,
                            13.,
                            MUTED,
                        );
                        if self.button(
                            "命名",
                            Rect::new(x + w - 128., y + 80. + i as f32 * 73., 50., 40.),
                            false,
                            true,
                        ) {
                            self.open_modal(Modal::Rename(r.id.clone(), r.title.clone().into()));
                        }
                        if self.button(
                            "删除",
                            Rect::new(x + w - 72., y + 80. + i as f32 * 73., 48., 40.),
                            false,
                            self.data.active.as_ref().is_none_or(|a| a.id != r.id),
                        ) {
                            self.open_modal(Modal::Delete(r.id.clone()));
                        }
                    }
                    if self.button(
                        "清理旧对局",
                        Rect::new(x + 24., y + 510., w - 48., 36.),
                        false,
                        !self.data.drafts.is_empty(),
                    ) {
                        self.open_modal(Modal::ClearDrafts);
                    }
                    if self.button(
                        "上一页",
                        Rect::new(x + 24., y + 460., (w - 56.) / 2., 40.),
                        false,
                        page > 0,
                    ) {
                        self.modal = Some(Modal::Drafts(page - 1));
                    }
                    if self.button(
                        "下一页",
                        Rect::new(x + w / 2. + 4., y + 460., (w - 56.) / 2., 40.),
                        false,
                        page + 1 < pages,
                    ) {
                        self.modal = Some(Modal::Drafts(page + 1));
                    }
                    if self.button(
                        "取消",
                        Rect::new(x + 24., y + 556., w - 48., 36.),
                        false,
                        true,
                    ) {
                        self.modal = None;
                    }
                }
            }
            Modal::Difficulty => {
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
                    "红方先行 · 进局后也可调整难度",
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
            Modal::Side(board) => {
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
            Modal::End(score) => {
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
            Modal::Delete(id) => {
                self.center("删除这局棋谱？", self.width / 2., y + 44., 26., INK);
                self.center("删除后无法恢复", self.width / 2., y + 103., 18., MUTED);
                self.modal = Some(Modal::Delete(id.clone()));
                if self.button(
                    "确认删除",
                    Rect::new(x + 24., y + 155., w - 48., 49.),
                    true,
                    true,
                ) {
                    self.enqueue(Command::Remove(id));
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
            Modal::Clear => {
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
                    self.enqueue(Command::Clear);
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
            Modal::Help => {
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
            Modal::About => {
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
            Modal::License(index) => {
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
        }
        if scale != 1. {
            self.width = old_canvas.0;
            self.height = old_canvas.1;
            self.click = self.click.map(|p| p / scale);
            if let Some((p, id)) = self.drag.take() {
                self.drag = Some((p / scale, id));
            }
            self.camera();
        }
    }
    fn wide_audio(&mut self) {
        draw_rectangle(
            0.,
            0.,
            self.width,
            self.height,
            Color::new(0.12, 0.17, 0.15, 0.48),
        );
        let w = (self.width - 64.).min(940.);
        let x = (self.width - w) / 2.;
        let y = (self.height - 438.) / 2.;
        self.card(Rect::new(x, y, w, 438.));
        self.text("声音设置", x + 28., y + 54., 28., INK);
        let mut changed = false;
        if self.button(
            if self.settings.sound {
                "总声音：开启"
            } else {
                "总声音：静音"
            },
            Rect::new(x + w - 228., y + 23., 200., 46.),
            false,
            true,
        ) {
            self.settings.sound = !self.settings.sound;
            changed = true;
        }
        let cw = (w - 76.) / 2.;
        for (i, label) in ["背景音乐", "棋子音效"].iter().enumerate() {
            let cx = x + 28. + i as f32 * (cw + 20.);
            let cy = y + 104.;
            self.panel(Rect::new(cx, cy, cw, 224.), PAPER);
            self.text(label, cx + 18., cy + 40., 25., INK);
            let enabled = if i == 0 {
                self.settings.music_enabled
            } else {
                self.settings.effects_enabled
            };
            if self.button(
                if enabled { "开启" } else { "关闭" },
                Rect::new(cx + cw - 88., cy + 16., 70., 40.),
                false,
                true,
            ) {
                if i == 0 {
                    self.settings.music_enabled = !enabled;
                } else {
                    self.settings.effects_enabled = !enabled;
                }
                changed = true;
            }
            let volume = if i == 0 {
                self.settings.music_volume
            } else {
                self.settings.effects_volume
            };
            self.center(
                &format!("{}%", (volume * 100.).round() as u32),
                cx + cw / 2.,
                cy + 109.,
                29.,
                JADE,
            );
            for (sign, bx, delta, allowed) in [
                ("-", cx + 18., -0.1, volume > 0.),
                ("+", cx + cw - 76., 0.1, volume < 1.),
            ] {
                if self.button(sign, Rect::new(bx, cy + 84., 58., 48.), false, allowed) {
                    let value = (volume + delta).clamp(0., 1.);
                    if i == 0 {
                        self.settings.music_volume = value;
                    } else {
                        self.settings.effects_volume = value;
                    }
                    changed = true;
                }
            }
            let tx = cx + 24.;
            let tw = cw - 48.;
            let ty = cy + 177.;
            let hit = Rect::new(tx - 8., ty - 22., tw + 16., 44.);
            let dragging = is_mouse_button_down(MouseButton::Left)
                && self
                    .drag
                    .as_ref()
                    .is_some_and(|(start, _)| hit.contains(*start));
            if self.click.is_some_and(|p| hit.contains(p)) || dragging {
                let pointer = self.click.unwrap_or_else(|| self.pointer());
                let value = ((pointer.x - tx) / tw).clamp(0., 1.);
                if i == 0 {
                    self.settings.music_volume = value;
                } else {
                    self.settings.effects_volume = value;
                }
                self.click = None;
                changed |= is_mouse_button_released(MouseButton::Left);
            }
            draw_line(tx, ty, tx + tw, ty, 5., BORDER);
            draw_line(
                tx,
                ty,
                tx + tw * volume,
                ty,
                5.,
                if self.settings.sound && enabled {
                    JADE
                } else {
                    GOLD
                },
            );
            draw_circle(tx + tw * volume, ty, 8., SURFACE);
            draw_circle(tx + tw * volume, ty, 6., JADE);
        }
        changed |= is_mouse_button_released(MouseButton::Left) && self.dragged;
        if changed {
            self.enqueue(Command::Settings(self.settings.clone()));
        }
        self.fit_text(
            if self.settings.sound {
                "落点定位 · 提示时音乐轻降"
            } else {
                "已静音，音量偏好仍会保留"
            },
            x + 28.,
            y + 384.,
            w - 264.,
            19.,
            MUTED,
        );
        self.modal = if self.button(
            "完成",
            Rect::new(x + w - 208., y + 356., 180., 54.),
            true,
            true,
        ) {
            None
        } else {
            Some(Modal::Audio)
        };
    }
    pub(super) fn modal_text(&mut self, text: &str, x: f32, y: f32, w: f32, h: f32, size: f32) {
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
