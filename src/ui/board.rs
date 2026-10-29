use super::*;
impl App {
    pub(super) fn warm_board(&mut self) {
        let started = std::time::Instant::now();
        let screen = self.screen;
        let size = (self.width, self.height);
        self.screen = Screen::Game;
        (self.width, self.height) = super::layout::canvas_size(
            self.viewport.w,
            self.viewport.h,
            self.viewport.w / self.viewport.h >= 1.35,
        );
        self.camera();
        self.board(&Board::default(), None, false);
        unsafe {
            macroquad::window::get_internal_gl().flush();
        }
        self.screen = screen;
        (self.width, self.height) = size;
        self.accessible.clear();
        self.semantic_counts.clear();
        self.camera();
        crate::diagnostics::measure("board_precache_ms", started.elapsed().as_secs_f64() * 1000.);
    }
    pub(super) fn board_rect(&self) -> (Vec2, f32) {
        let (x, y, cell) =
            super::layout::board_geometry(self.width, self.height, self.screen == Screen::Review);
        (vec2(x, y), cell)
    }
    pub(super) fn point(origin: Vec2, cell: f32, s: Square, flip: bool) -> Vec2 {
        let (f, r) = if flip {
            (8 - s.file(), 9 - s.rank())
        } else {
            (s.file(), s.rank())
        };
        origin + vec2(f as f32 * cell, r as f32 * cell)
    }
    pub(super) fn board(&mut self, board: &Board, last: Option<Move>, interactive: bool) {
        let (origin, cell) = self.board_rect();
        let flip = if self.screen == Screen::Review {
            self.review.as_ref().is_some_and(|r| !r.user_red)
        } else {
            !self.settings.user_red
        };
        for rank in 0..10 {
            for file in 0..9 {
                let square = Square::new(file, rank).unwrap();
                let at = Self::point(origin, cell, square, flip);
                let label = board.at(square).map_or_else(
                    || "空位".into(),
                    |p| format!("{}{}", p.side.label(), p.label()),
                );
                self.semantic_named(
                    &format!("square:{file}:{rank}"),
                    format!(
                        "第{}列第{}行，{}{}",
                        if flip { 9 - file } else { file + 1 },
                        if flip { 10 - rank } else { rank + 1 },
                        label,
                        if self.selected == Some(square) {
                            "，已选中"
                        } else {
                            ""
                        }
                    ),
                    Rect::new(
                        at.x - cell * 0.45,
                        at.y - cell * 0.45,
                        cell * 0.9,
                        cell * 0.9,
                    ),
                    interactive,
                );
            }
        }
        self.cached_board_surface(origin, cell, flip);
        if let Some(m) = last {
            for s in [m.from, m.to] {
                let p = Self::point(origin, cell, s, flip);
                draw_rectangle(
                    p.x - cell * 0.44,
                    p.y - cell * 0.44,
                    cell * 0.88,
                    cell * 0.88,
                    Color::new(JADE.r, JADE.g, JADE.b, 0.16),
                );
            }
        }
        let mut hints = self.hints.clone();
        if self.screen == Screen::Review
            && let Some(r) = &self.review
            && let Some(m) = r.bests[r.ply]
        {
            hints.push(m);
        }
        for (i, hint) in hints.iter().enumerate() {
            let a = Self::point(origin, cell, hint.from, flip);
            let b = Self::point(origin, cell, hint.to, flip);
            let color = if i == 0 {
                Color::new(0.15, 0.5, 0.35, 0.8)
            } else {
                Color::new(0.1, 0.35, 0.7, 0.8)
            };
            draw_line(a.x, a.y, b.x, b.y, 4., color);
            let v = (b - a).normalize_or_zero();
            let perp = vec2(-v.y, v.x);
            draw_triangle(b, b - v * 16. + perp * 7., b - v * 16. - perp * 7., color);
            let mid = (a + b) / 2.;
            draw_circle(mid.x, mid.y, 11., color);
            self.center(if i == 0 { "优" } else { "良" }, mid.x, mid.y, 15., PAPER);
        }
        for (s, piece) in board.pieces() {
            if self.screen == Screen::Game && self.animation.as_ref().is_some_and(|a| a.mv.to == s)
            {
                continue;
            }
            let p = Self::point(origin, cell, s, flip);
            if piece.kind == crate::rules::Kind::King && board.in_check(piece.side) {
                draw_circle(p.x, p.y, cell * 0.49, Color::new(0.8, 0.12, 0.08, 0.4));
            }
            self.chess_piece(p, cell, piece);
        }
        if self.screen == Screen::Game
            && let Some(a) = &self.animation
        {
            let t = ((get_time() - a.started) / 0.26).clamp(0., 1.) as f32;
            let smooth = t * t * (3. - 2. * t);
            if let Some(captured) = a.captured {
                let p = Self::point(origin, cell, a.mv.to, flip);
                draw_circle(
                    p.x,
                    p.y,
                    cell * 0.4,
                    Color::new(PAPER.r, PAPER.g, PAPER.b, 1. - t),
                );
                self.center(
                    captured.label(),
                    p.x,
                    p.y,
                    cell * 0.5,
                    Color::new(INK.r, INK.g, INK.b, 1. - t),
                );
            }
            let p = Self::point(origin, cell, a.mv.from, flip)
                .lerp(Self::point(origin, cell, a.mv.to, flip), smooth);
            self.chess_piece(p, cell, a.piece);
        }
        if self.screen == Screen::Review
            && let Some(r) = &self.review
            && r.ply > 0
            && let Some(assessment) = r.assessment(r.ply)
        {
            let e = &r.game.history[r.ply - 1];
            let p = Self::point(origin, cell, e.mv.to, flip) + vec2(cell * 0.3, -cell * 0.3);
            draw_circle(p.x, p.y, 10., RED_INK);
            self.center(assessment.badge(), p.x, p.y, 14., PAPER);
        }
        if let Some(s) = self.selected {
            let p = Self::point(origin, cell, s, flip);
            draw_circle_lines(p.x, p.y, cell * 0.46, 3., RED_INK);
            for m in self.legal_moves(board).into_iter().filter(|m| m.from == s) {
                let p = Self::point(origin, cell, m.to, flip);
                if board.at(m.to).is_some() {
                    draw_circle_lines(p.x, p.y, cell * 0.46, 2.5, JADE);
                } else {
                    draw_circle(p.x, p.y, cell * 0.12, JADE);
                    draw_circle_lines(
                        p.x,
                        p.y,
                        cell * 0.21,
                        1.,
                        Color::new(JADE.r, JADE.g, JADE.b, 0.4),
                    );
                }
            }
        }
        if interactive && self.keyboard_active {
            if is_key_pressed(KeyCode::B) {
                self.board_cursor = Square::new(4, if flip { 0 } else { 9 });
            }
            if let Some(mut square) = self.board_cursor {
                let mut df = 0;
                let mut dr = 0;
                if is_key_pressed(KeyCode::Left) {
                    df -= 1;
                }
                if is_key_pressed(KeyCode::Right) {
                    df += 1;
                }
                if is_key_pressed(KeyCode::Up) {
                    dr -= 1;
                }
                if is_key_pressed(KeyCode::Down) {
                    dr += 1;
                }
                let sign = if flip { -1 } else { 1 };
                square = Square::new(
                    (square.file() + df * sign).clamp(0, 8),
                    (square.rank() + dr * sign).clamp(0, 9),
                )
                .unwrap();
                self.board_cursor = Some(square);
                let at = Self::point(origin, cell, square, flip);
                let unit = self.width / self.viewport.w;
                let native = vec2(self.viewport.x + at.x / unit, self.viewport.y + at.y / unit);
                self.accessibility_focus = self
                    .accessible
                    .iter()
                    .find(|e| {
                        Rect::new(e.rect[0], e.rect[1], e.rect[2], e.rect[3]).contains(native)
                    })
                    .map(|e| e.id);
                draw_rectangle_lines(
                    at.x - cell * 0.46,
                    at.y - cell * 0.46,
                    cell * 0.92,
                    cell * 0.92,
                    2.,
                    JADE,
                );
                if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter) {
                    self.click = Some(at);
                }
            }
        }
        if interactive
            && let Some(p) = self.click
            && let Some(s) =
                crate::interaction::board_hit([p.x, p.y], [origin.x, origin.y], cell, flip)
        {
            self.click = None;
            if board.at(s).is_some_and(|p| p.side == self.player()) {
                self.selected = if self.selected == Some(s) {
                    None
                } else {
                    Some(s)
                };
                self.hints.clear();
            } else if let Some(from) = self.selected {
                let m = Move { from, to: s };
                if board.is_legal(m) {
                    self.play(m);
                }
            }
        }
    }
    fn cached_board_surface(&self, origin: Vec2, cell: f32, flip: bool) {
        let width = cell * 10.;
        let height = cell * 11.4 + 8.;
        let density = self.scene.scale().y * self.viewport.h / self.height;
        let pixels = (width * density).ceil().clamp(32., 4096.) as u16;
        let cached = self
            .board_surface_cache
            .borrow()
            .as_ref()
            .filter(|(flipped, size, _)| *flipped == flip && *size == pixels)
            .map(|(_, _, target)| target.clone());
        let target = cached.unwrap_or_else(|| {
            let target = render_target_ex(
                pixels.into(),
                (f32::from(pixels) * height / width).ceil() as u32,
                RenderTargetParams {
                    sample_count: 1,
                    depth: false,
                },
            );
            target.texture.set_filter(FilterMode::Linear);
            let mut camera = Camera2D::from_display_rect(Rect::new(0., 0., width, height));
            camera.render_target = Some(target.clone());
            set_camera(&camera);
            self.scene.use_scene_material();
            clear_background(Color::new(0., 0., 0., 0.));
            self.font.set_pixel_density(f32::from(pixels) / width);
            self.draw_board_surface(vec2(cell, cell * 1.2), cell, flip);
            self.camera();
            unsafe {
                macroquad::window::get_internal_gl().flush();
            }
            *self.board_surface_cache.borrow_mut() = Some((flip, pixels, target.clone()));
            target
        });
        draw_texture_ex(
            &target.texture,
            origin.x - cell,
            origin.y - cell * 1.2,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(width, height)),
                flip_y: true,
                ..Default::default()
            },
        );
    }
    fn draw_board_surface(&self, origin: Vec2, cell: f32, flip: bool) {
        let frame = Rect::new(
            origin.x - cell,
            origin.y - cell * 1.2,
            cell * 10.,
            cell * 11.4,
        );
        self.panel(
            Rect::new(frame.x, frame.y + 5., frame.w, frame.h),
            Color::new(0.22, 0.12, 0.05, 0.18),
        );
        self.panel(frame, Color::new(0.38, 0.31, 0.22, 1.));
        self.wood_surface(Rect::new(
            frame.x + 5.,
            frame.y + 5.,
            frame.w - 10.,
            frame.h - 10.,
        ));
        for x in [frame.x + 11., frame.x + frame.w - 11.] {
            for y in [frame.y + 11., frame.y + frame.h - 11.] {
                draw_circle(x, y, 2., GOLD);
            }
        }
        draw_rectangle_lines(
            origin.x - cell * 0.5,
            origin.y - cell * 0.5,
            cell * 9.,
            cell * 10.,
            2.,
            GOLD,
        );
        draw_rectangle_lines(
            origin.x - cell * 0.44,
            origin.y - cell * 0.44,
            cell * 8.88,
            cell * 9.88,
            1.,
            MUTED,
        );
        for r in 0..10 {
            let y = origin.y + r as f32 * cell;
            draw_line(origin.x, y, origin.x + cell * 8., y, 1.2, INK);
        }
        for f in 0..9 {
            let x = origin.x + f as f32 * cell;
            if f == 0 || f == 8 {
                draw_line(x, origin.y, x, origin.y + 9. * cell, 1.2, INK);
            } else {
                draw_line(x, origin.y, x, origin.y + 4. * cell, 1.2, INK);
                draw_line(x, origin.y + 5. * cell, x, origin.y + 9. * cell, 1.2, INK);
            }
        }
        for r in [0., 7.] {
            draw_line(
                origin.x + 3. * cell,
                origin.y + r * cell,
                origin.x + 5. * cell,
                origin.y + (r + 2.) * cell,
                1.2,
                INK,
            );
            draw_line(
                origin.x + 5. * cell,
                origin.y + r * cell,
                origin.x + 3. * cell,
                origin.y + (r + 2.) * cell,
                1.2,
                INK,
            );
        }
        // Traditional cannon and pawn registration marks, scaled with the board.
        for (file, rank) in [
            (1, 2),
            (7, 2),
            (1, 7),
            (7, 7),
            (0, 3),
            (2, 3),
            (4, 3),
            (6, 3),
            (8, 3),
            (0, 6),
            (2, 6),
            (4, 6),
            (6, 6),
            (8, 6),
        ] {
            let p = origin + vec2(file as f32 * cell, rank as f32 * cell);
            for sx in [-1., 1.] {
                if (file == 0 && sx < 0.) || (file == 8 && sx > 0.) {
                    continue;
                }
                for sy in [-1., 1.] {
                    let a = p + vec2(sx * cell * 0.10, sy * cell * 0.10);
                    draw_line(a.x, a.y, a.x + sx * cell * 0.13, a.y, 1., MUTED);
                    draw_line(a.x, a.y, a.x, a.y + sy * cell * 0.13, 1., MUTED);
                }
            }
        }
        self.center(
            "楚 河",
            origin.x + 2. * cell,
            origin.y + 4.5 * cell,
            cell * 0.52,
            MUTED,
        );
        self.center(
            "汉 界",
            origin.x + 6. * cell,
            origin.y + 4.5 * cell,
            cell * 0.52,
            MUTED,
        );
        for f in 0..9 {
            const CN: [&str; 9] = ["一", "二", "三", "四", "五", "六", "七", "八", "九"];
            self.center(
                &if flip {
                    CN[f].into()
                } else {
                    (f + 1).to_string()
                },
                origin.x + f as f32 * cell,
                origin.y - cell * 0.78,
                (cell * 0.42).min(15.),
                if flip { RED_INK } else { MUTED },
            );
            self.center(
                &if flip {
                    (9 - f).to_string()
                } else {
                    CN[8 - f].into()
                },
                origin.x + f as f32 * cell,
                origin.y + 9.65 * cell,
                (cell * 0.44).min(16.),
                if flip { MUTED } else { RED_INK },
            );
        }
    }
    fn chess_piece(&self, p: Vec2, cell: f32, piece: Piece) {
        let side = cell * 1.3;
        let pixels = (side * self.scene.scale().y * self.viewport.h / self.height)
            .ceil()
            .clamp(16., 512.) as u16;
        let cached = self
            .piece_cache
            .borrow()
            .iter()
            .find(|(kind, size, _)| *kind == piece && *size == pixels)
            .map(|(_, _, target)| target.clone());
        let target = cached.unwrap_or_else(|| {
            let target = render_target_ex(
                pixels.into(),
                pixels.into(),
                RenderTargetParams {
                    sample_count: 1,
                    depth: false,
                },
            );
            target.texture.set_filter(FilterMode::Linear);
            let mut camera = Camera2D::from_display_rect(Rect::new(0., 0., side, side));
            camera.render_target = Some(target.clone());
            set_camera(&camera);
            self.scene.use_scene_material();
            clear_background(Color::new(0., 0., 0., 0.));
            self.font.set_pixel_density(f32::from(pixels) / side);
            self.chess_piece_uncached(vec2(side / 2., side / 2.), cell, piece);
            self.camera();
            let mut cache = self.piece_cache.borrow_mut();
            if cache.len() >= 28 {
                unsafe {
                    macroquad::window::get_internal_gl().flush();
                }
                cache.pop_front();
            }
            cache.push_back((piece, pixels, target.clone()));
            target
        });
        draw_texture_ex(
            &target.texture,
            p.x - side / 2.,
            p.y - side / 2.,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(side, side)),
                flip_y: true,
                ..Default::default()
            },
        );
    }
    fn chess_piece_uncached(&self, p: Vec2, cell: f32, piece: Piece) {
        let radius = cell * 0.42;
        let color = if piece.side == Side::Red {
            RED_INK
        } else {
            INK
        };
        draw_circle(
            p.x + 1.,
            p.y + cell * 0.065,
            radius * 1.03,
            Color::new(0.26, 0.15, 0.07, 0.23),
        );
        draw_circle(p.x, p.y + 1., radius, Color::new(0.55, 0.37, 0.19, 1.));
        draw_circle(
            p.x,
            p.y - 0.6,
            radius * 0.97,
            Color::new(0.99, 0.92, 0.76, 1.),
        );
        draw_circle(
            p.x,
            p.y + 0.5,
            radius * 0.88,
            Color::new(0.88, 0.76, 0.56, 1.),
        );
        draw_circle(
            p.x,
            p.y - 0.5,
            radius * 0.85,
            Color::new(0.98, 0.92, 0.79, 1.),
        );
        draw_circle_lines(p.x, p.y - 0.5, radius * 0.82, 1.2, color);
        draw_circle_lines(
            p.x,
            p.y - 0.5,
            radius * 0.72,
            0.6,
            Color::new(color.r, color.g, color.b, 0.32),
        );
        self.center(piece.label(), p.x, p.y - 0.6, cell * 0.53, color);
    }
}
