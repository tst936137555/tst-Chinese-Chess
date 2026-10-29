use super::*;
impl App {
    pub(super) fn modal_audio(&mut self, x: f32, y: f32, w: f32) {
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
            let wheel = self.wheel().1;
            if track.contains(pointer) && wheel != 0. {
                let value = crate::interaction::wheel_volume(volume, wheel);
                if i == 0 {
                    self.settings.music_volume = value;
                } else {
                    self.settings.effects_volume = value;
                }
                changed = true;
            }
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
            self.save_preferences();
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
    pub(super) fn wide_audio(&mut self) {
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
            let pointer = self.pointer();
            let wheel = self.wheel().1;
            if hit.contains(pointer) && wheel != 0. {
                let value = crate::interaction::wheel_volume(volume, wheel);
                if i == 0 {
                    self.settings.music_volume = value;
                } else {
                    self.settings.effects_volume = value;
                }
                changed = true;
            }
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
            self.save_preferences();
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
}
