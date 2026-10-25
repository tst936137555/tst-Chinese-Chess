use super::*;
impl App {
    pub(super) fn modal_rename(
        &mut self,
        id: String,
        mut edit: crate::text_input::NameEdit,
        compact_name: bool,
        x: f32,
        y: f32,
        w: f32,
    ) {
        if let Some((text, start, end)) = crate::text_input::take_native_edit() {
            edit.replace_native(text, start, end);
        }
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
        let unit = self.width / self.viewport.w;
        let native_rect = [
            self.viewport.x + field.x / unit,
            self.viewport.y + field.y / unit,
            field.w / unit,
            field.h / unit,
        ];
        let selection = edit.selection();
        crate::platform::name_editor(
            &edit.text,
            selection.start,
            selection.end,
            native_rect,
            self.name_keyboard,
        );
        macroquad::miniquad::window::set_ime_position(
            ((native_rect[0] + native_rect[2]) * screen_dpi_scale()) as i32,
            ((native_rect[1] + native_rect[3]) * screen_dpi_scale()) as i32,
        );
        self.panel(field, PAPER);
        self.semantic("名称输入".into(), field, true);
        if let Some(element) = self.accessible.last_mut() {
            element.value = Some(edit.text.clone());
            if self.name_keyboard {
                self.accessibility_focus = Some(element.id);
            }
        }
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
                            &(self.font.measure(&prefix(start, b), 20.).width - (click.x - text_x))
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
        if !cfg!(target_os = "ios")
            && matches!(
                self.font.display_text(&edit.text),
                std::borrow::Cow::Owned(_)
            )
        {
            self.fit_text(
                "部分字符显示为 Unicode 编号，原文会完整保存",
                x + 24.,
                y + field_y + 74.,
                w - 48.,
                12.,
                MUTED,
            );
        }
        if self.name_keyboard && ((get_time() * 2.) as u64).is_multiple_of(2) {
            let cursor_x = text_x + self.font.measure(&prefix(start, edit.cursor), 20.).width;
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
        for (i, label) in ["开局练习", "残局练习", "待续对局"].iter().enumerate() {
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
            || crate::text_input::take_native_done()
        {
            self.enqueue(Command::Rename(id, edit.text, self.rename_revision));
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
}
