use super::*;

/// Small, deterministic textures uploaded once; no per-frame noise or file IO.
pub(super) struct Surfaces {
    paper: Texture2D,
    wood: Texture2D,
}
impl Surfaces {
    pub(super) fn new() -> Self {
        let texture = |wood: bool| {
            let size = if wood { 1024u16 } else { 512u16 };
            let mut image = Image::gen_image_color(size, size, WHITE);
            let mut seed = 0x1357_2468u32;
            for y in 0..size {
                for x in 0..size {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    let noise = (seed >> 24) as f32 / 255. - 0.5;
                    let xf = x as f32 * 256. / f32::from(size);
                    let yf = y as f32 * 256. / f32::from(size);
                    let (base, variation) = if wood {
                        let grain = (yf * 0.72 + (xf * 0.025).sin() * 2.4).sin();
                        let broad = (yf * 0.09 + (xf * 0.018).sin()).sin();
                        (WOOD, noise * 0.008 + grain * 0.008 + broad * 0.012)
                    } else {
                        (PAPER, noise * 0.004)
                    };
                    image.set_pixel(
                        x as u32,
                        y as u32,
                        Color::new(
                            base.r + variation,
                            base.g + variation,
                            base.b + variation,
                            1.,
                        ),
                    );
                }
            }
            let result = Texture2D::from_image(&image);
            result.set_filter(FilterMode::Linear);
            result
        };
        Self {
            paper: texture(false),
            wood: texture(true),
        }
    }
}
impl App {
    pub(super) fn background(&self) {
        draw_texture_ex(
            &self.surfaces.paper,
            0.,
            0.,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(self.width, self.height)),
                ..Default::default()
            },
        );
    }
    pub(super) fn wood_surface(&self, r: Rect) {
        draw_texture_ex(
            &self.surfaces.wood,
            r.x,
            r.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(r.w, r.h)),
                ..Default::default()
            },
        );
    }
    pub(super) fn card(&self, r: Rect) {
        self.panel(
            Rect::new(r.x, r.y + 3., r.w, r.h),
            Color::new(0.20, 0.24, 0.22, 0.035),
        );
        self.panel(r, BORDER);
        self.panel(Rect::new(r.x + 1., r.y + 1., r.w - 2., r.h - 2.), SURFACE);
    }
    pub(super) fn panel(&self, r: Rect, color: Color) {
        let rad = 14_f32.min(r.h / 2.).min(r.w / 2.);
        draw_rectangle(r.x + rad, r.y, r.w - 2. * rad, r.h, color);
        draw_rectangle(r.x, r.y + rad, r.w, r.h - 2. * rad, color);
        for (x, y) in [
            (r.x + rad, r.y + rad),
            (r.x + r.w - rad, r.y + rad),
            (r.x + rad, r.y + r.h - rad),
            (r.x + r.w - rad, r.y + r.h - rad),
        ] {
            draw_circle(x, y, rad, color);
        }
    }
    pub(super) fn fit_text(
        &self,
        text: &str,
        x: f32,
        y: f32,
        max_width: f32,
        size: f32,
        color: Color,
    ) {
        if max_width <= 0. {
            return;
        }
        if self.font.measure(text, size).width <= max_width {
            self.text(text, x, y, size, color);
            return;
        }
        let mut shown = String::new();
        for c in text.chars() {
            let mut next = shown.clone();
            next.push(c);
            if self.font.measure(&(next.clone() + "…"), size).width > max_width {
                shown.push('…');
                break;
            }
            shown = next;
        }
        self.text(&shown, x, y, size, color);
    }
    pub(super) fn player(&self) -> Side {
        if self.settings.user_red {
            Side::Red
        } else {
            Side::Black
        }
    }
    pub(super) fn text(&self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        self.font.draw(text, x, y, size, color);
    }
    pub(super) fn center(&self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        let m = self.font.measure(text, size);
        self.text(text, x - m.width / 2., y + size * 0.32, size, color);
    }
    pub(super) fn button(&mut self, label: &str, r: Rect, primary: bool, enabled: bool) -> bool {
        // Hit areas use viewport units; keep short-screen controls reachable without shrinking text.
        let unit = self.width / self.viewport.w;
        let area = crate::interaction::touch_area([r.x, r.y, r.w, r.h], unit);
        let hit_rect = Rect::new(area[0], area[1], area[2], area[3]);
        let index = self.focus_count;
        if enabled && self.keyboard_active {
            self.focus_count += 1;
            if index == self.focus_index && self.keyboard_nav && self.board_cursor.is_none() {
                self.focus = Some(hit_rect);
                draw_rectangle_lines(r.x - 2., r.y - 2., r.w + 4., r.h + 4., 1., JADE);
            }
            if crate::platform::observation_path().is_some() {
                self.observed_controls.push((
                    label.into(),
                    Rect::new(
                        self.viewport.x + hit_rect.x / unit,
                        self.viewport.y + hit_rect.y / unit,
                        hit_rect.w / unit,
                        hit_rect.h / unit,
                    ),
                ));
            }
        }
        let mouse = self.pointer();
        let hover = enabled && r.contains(mouse);
        let pressed = hover && is_mouse_button_down(MouseButton::Left);
        let color = if !enabled {
            Color::new(0.93, 0.93, 0.90, 1.)
        } else if pressed {
            if primary {
                Color::new(0.54, 0.18, 0.14, 1.)
            } else {
                Color::new(0.86, 0.89, 0.85, 1.)
            }
        } else if hover {
            if primary {
                Color::new(0.72, 0.28, 0.22, 1.)
            } else {
                Color::new(0.94, 0.96, 0.92, 1.)
            }
        } else if primary {
            RED_INK
        } else {
            SURFACE
        };
        if enabled && !pressed {
            self.panel(
                Rect::new(r.x, r.y + 2., r.w, r.h),
                Color::new(0.20, 0.24, 0.22, 0.045),
            );
        }
        self.panel(r, if primary && enabled { RED_INK } else { BORDER });
        let r = Rect::new(r.x + 1., r.y + 1., r.w - 2., r.h - 2.);
        self.panel(r, color);
        let ink = if primary && enabled {
            SURFACE
        } else if enabled {
            INK
        } else {
            Color::new(0.62, 0.64, 0.59, 1.)
        };
        if label == "\u{2605}" || label == "\u{2606}" {
            let center = vec2(r.x + r.w / 2., r.y + r.h / 2.);
            let radius = 10.;
            let points: Vec<_> = (0..10)
                .map(|i| {
                    let angle = i as f32 * std::f32::consts::PI / 5. - std::f32::consts::FRAC_PI_2;
                    center
                        + vec2(angle.cos(), angle.sin())
                            * if i % 2 == 0 { radius } else { radius * 0.43 }
                })
                .collect();
            for i in 0..10 {
                let a = points[i];
                let b = points[(i + 1) % 10];
                if label == "\u{2605}" {
                    draw_triangle(center, a, b, RED_INK);
                } else {
                    draw_line(a.x, a.y, b.x, b.y, 1.5, ink);
                }
            }
        } else if label == "-" {
            // The bundled subset has no U+2212 glyph. Geometry stays correct in every font.
            let cx = r.x + r.w / 2.;
            let cy = r.y + r.h / 2.;
            draw_line(cx - 6., cy, cx + 6., cy, 2., ink);
        } else {
            let size = 20_f32.min((r.w - 12.) / label.chars().count().max(1) as f32);
            self.center(label, r.x + r.w / 2., r.y + r.h / 2., size, ink);
        }
        if enabled
            && (self.click.is_some_and(|p| hit_rect.contains(p))
                || (self.keyboard_active
                    && self.board_cursor.is_none()
                    && index == self.focus_index
                    && is_key_pressed(KeyCode::Enter)))
        {
            self.click = None;
            true
        } else {
            false
        }
    }
}
