use super::*;
pub async fn run() {
    // Startup also awaits frames; the blocking Windows loop must wake throughout loading.
    let wake = crate::frame_wake::FrameWake::default();
    let (mut app, mut smoke, mut profile) = startup::initialize().await;
    prevent_quit();
    let subscriber = macroquad::input::utils::register_input_subscriber();
    let mut lifecycle = Lifecycle;
    let mut previous_mouse = mouse_position();
    let mut previous_blink = 0;
    loop {
        let frame_started = std::time::Instant::now();
        macroquad::input::utils::repeat_all_miniquad_input(&mut lifecycle, subscriber);
        if let Some(s) = &mut smoke {
            s.setup(&mut app);
        }
        if let Some(profile) = &mut profile {
            profile.setup(&mut app);
        }
        app.handle_lifecycle();
        app.poll();
        if is_quit_requested() && app.close == crate::session::CloseState::Open {
            app.save_review_cache();
            app.cancel();
            app.close.request();
            app.notice = "正在保存后退出...".into();
            app.enqueue(Command::Flush);
        }
        if app
            .close
            .can_exit(app.saves.pending, !app.saves.failed.is_empty())
        {
            break;
        }
        if app.close.waiting() && !app.saves.failed.is_empty() {
            app.close.cancel();
            app.open_modal(Modal::SaveFailed);
        }
        app.audio.prepare().await;
        app.audio.tick(&app.settings, get_frame_time(), get_time());
        let [top, right, bottom, left] = crate::platform::safe_insets();
        let old_viewport = app.viewport;
        app.viewport = Rect::new(
            left,
            top,
            (screen_width() - left - right).max(1.),
            (screen_height() - top - bottom).max(1.),
        );
        if (old_viewport.w - app.viewport.w).abs() > 1.
            || (old_viewport.h - app.viewport.h).abs() > 1.
            || old_viewport.x != app.viewport.x
            || old_viewport.y != app.viewport.y
        {
            app.redraw = true;
            app.move_layout_key.1 = u64::MAX;
            app.drag = None;
            app.dragged = false;
            app.click = None;
        }
        let ratio = app.viewport.w / app.viewport.h;
        let split = ratio >= 1.35
            && matches!(app.screen, Screen::Game | Screen::Review)
            && (app.screen == Screen::Review || app.game.outcome == Outcome::Playing);
        (app.width, app.height) = layout::canvas_size(app.viewport.w, app.viewport.h, split);
        app.redraw |= app.scene.resize();
        app.camera();
        let mouse = app.pointer();
        // Release semantics allow a left swipe in the archive without accidentally opening it.
        if is_mouse_button_pressed(MouseButton::Left) {
            app.drag = Some((mouse, None));
            app.dragged = false;
        }
        if is_mouse_button_down(MouseButton::Left)
            && app
                .drag
                .as_ref()
                .is_some_and(|(p, _)| p.distance(mouse) > 18.)
        {
            app.dragged = true;
        }
        app.click = if is_mouse_button_released(MouseButton::Left) {
            app.drag
                .as_ref()
                .filter(|(p, _)| !app.dragged && p.distance(mouse) < 18.)
                .map(|_| mouse)
        } else {
            None
        };
        if app.close.waiting() {
            app.click = None;
        }
        if let Some(action) = crate::accessibility::take_action() {
            app.redraw = true;
            let id = match action {
                crate::accessibility::ControlAction::Click(id)
                | crate::accessibility::ControlAction::Focus(id) => id,
            };
            if let Some(element) = app.accessible.iter().find(|e| e.id == id && !e.disabled) {
                let [x, y, w, h] = element.rect;
                let point = crate::interaction::map_point(
                    [x + w / 2., y + h / 2.],
                    [
                        app.viewport.x,
                        app.viewport.y,
                        app.viewport.w,
                        app.viewport.h,
                    ],
                    [app.width, app.height],
                );
                match action {
                    crate::accessibility::ControlAction::Click(_) if element.actionable => {
                        app.click = Some(vec2(point[0], point[1]))
                    }
                    crate::accessibility::ControlAction::Focus(_) => {
                        if element.value.is_some() {
                            app.name_keyboard = true;
                        } else {
                            let (origin, cell) = app.board_rect();
                            let square = if app.screen == Screen::Game && app.modal.is_none() {
                                crate::interaction::board_hit(
                                    point,
                                    [origin.x, origin.y],
                                    cell,
                                    !app.settings.user_red,
                                )
                            } else {
                                None
                            };
                            if square.is_some() {
                                app.board_cursor = square;
                            } else {
                                app.focus_request = Some(id);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if app
            .animation
            .as_ref()
            .is_some_and(|a| get_time() - a.started >= 0.26)
        {
            app.animation = None;
            app.redraw = true;
        }
        if let Some(s) = &smoke {
            s.input(&mut app);
        }
        let blink = (get_time() * 2.) as u64;
        let input_changed = !get_keys_pressed().is_empty()
            || !get_keys_released().is_empty()
            || is_mouse_button_pressed(MouseButton::Left)
            || is_mouse_button_released(MouseButton::Left)
            || is_mouse_button_down(MouseButton::Left)
            || mouse_wheel() != (0., 0.);
        let paint = !crate::platform::suspended()
            && (smoke.is_some()
                || (matches!(app.modal, Some(Modal::Rename(..)))
                    && crate::text_input::has_native_input())
                || app.redraw
                || input_changed
                || previous_mouse != mouse_position()
                || app.animation.is_some()
                || (app.name_keyboard && blink != previous_blink)
                || (app.screen == Screen::Game
                    && app.game.outcome != Outcome::Playing
                    && !app.end_ready));
        previous_mouse = mouse_position();
        previous_blink = blink;
        if paint {
            app.redraw = input_changed;
            app.focus_count = 0;
            app.observed_controls.clear();
            app.accessible.clear();
            app.semantic_counts.clear();
            app.semantic_scope.clear();
            app.accessibility_focus = None;
            clear_background(PAPER);
            app.background();
            let modal_click = if app.modal.is_some()
                || (app.screen == Screen::Game && app.game.outcome != Outcome::Playing)
            {
                app.click.take()
            } else {
                None
            };
            // The failure bar is drawn above page controls; reserve its pointer input first.
            let notice_click = if app.notice_contains(app.click) {
                app.click.take()
            } else {
                None
            };
            app.keyboard_active = app.modal.is_none()
                && !(app.screen == Screen::Game && app.game.outcome != Outcome::Playing);
            app.accessibility_active = app.keyboard_active;
            app.header();
            match app.screen {
                Screen::Home => app.home(),
                Screen::Game => app.play_screen(),
                Screen::Archives => app.archives(),
                Screen::Review => app.review_screen(),
            }
            if app.screen == Screen::Game
                && app.game.outcome != Outcome::Playing
                && app.modal.is_none()
            {
                app.accessible.clear();
                app.accessibility_active = true;
                app.keyboard_active = true;
                app.click = modal_click;
                app.end_overlay();
            }
            if app.modal.is_some() {
                app.accessible.clear();
                app.accessibility_active = true;
                app.keyboard_active = true;
                app.click = modal_click;
                app.draw_modal();
            }
            if app.modal.is_none() && notice_click.is_some() {
                app.click = notice_click;
            }
            app.draw_notice();
            crate::diagnostics::measure(
                "scene_draw_ms",
                frame_started.elapsed().as_secs_f64() * 1000.,
            );
        } else {
            crate::diagnostics::measure("scene_reused", 1.);
        }
        if smoke.is_none()
            && !app.close.waiting()
            && app.screen == Screen::Game
            && app.modal.is_none()
            && app.pending.is_none()
            && app.animation.is_none()
            && !app.engine_failed
            && !crate::platform::suspended()
            && app.game.outcome == Outcome::Playing
            && app.game.board.turn != app.player()
        {
            app.search(Job::Move);
        }
        let native_back = crate::platform::take_back();
        if is_key_pressed(KeyCode::Escape) || native_back {
            app.redraw = true;
            if native_back && app.screen == Screen::Home && app.modal.is_none() {
                macroquad::miniquad::window::request_quit();
            } else if app.modal.is_some() {
                app.accessible.clear();
                if matches!(app.modal, Some(Modal::End(_))) {
                    app.cancel();
                }
                if matches!(app.modal, Some(Modal::Rename(..))) {
                    app.finish_rename();
                } else {
                    app.modal = None;
                }
            } else if app.screen == Screen::Game
                && (app.selected.is_some() || app.board_cursor.is_some())
            {
                app.board_cursor = None;
                app.selected = None;
                app.hints.clear();
            } else {
                app.back();
            }
        }
        let present_started = std::time::Instant::now();
        app.scene.present();
        crate::diagnostics::measure(
            "present_submit_ms",
            present_started.elapsed().as_secs_f64() * 1000.,
        );
        let summary = match &app.modal {
            Some(Modal::Rename(_, edit)) => format!(
                "名称编辑：{}。已选中 {} 个字",
                edit.text,
                edit.selection().len()
            ),
            Some(_) => "中国象棋，对话框".into(),
            None => match app.screen {
                Screen::Game => {
                    let mut summary = format!(
                        "{}，{}行棋",
                        app.game.outcome.label(),
                        app.game.board.turn.label()
                    );
                    if !app.saves.failed.is_empty() {
                        summary.push_str("，保存失败，可处理保存");
                    }
                    summary
                }
                Screen::Review => "复盘，可逐步查看棋盘和分析结果".into(),
                Screen::Archives => "棋谱收藏".into(),
                Screen::Home => "中国象棋主页".into(),
            },
        };
        crate::accessibility::publish(
            format!("{summary}。{}", app.notice),
            app.accessible.clone(),
            screen_dpi_scale(),
            app.accessibility_focus,
        );
        if let Some(s) = &mut smoke
            && s.capture(&mut app)
        {
            macroquad::miniquad::window::request_quit();
            smoke = None;
        }
        if let Some(profiler) = &mut profile
            && profiler.capture(&app, paint)
        {
            macroquad::miniquad::window::request_quit();
            profile = None;
        }
        if is_key_pressed(KeyCode::Tab) {
            app.keyboard_nav = true;
            app.board_cursor = None;
            let count = app.focus_count.max(1);
            app.focus_index = if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)
            {
                (app.focus_index + count - 1) % count
            } else {
                (app.focus_index + 1) % count
            };
        }
        app.focus_index %= app.focus_count.max(1);
        if app.modal.is_none() && is_key_pressed(KeyCode::M) {
            app.settings.sound = !app.settings.sound;
            app.save_preferences();
        }
        crate::diagnostics::measure("frame_ms", f64::from(get_frame_time()) * 1000.);
        let work_ms = frame_started.elapsed().as_secs_f32() * 1000.;
        crate::diagnostics::measure("frame_work_ms", f64::from(work_ms));
        if paint && smoke.is_none() && !crate::platform::suspended() {
            // Event-loop waiting is not rendering pressure.
            app.scene.budget.observe(work_ms, get_frame_time());
        }
        if crate::platform::smoke_output().is_none() && smoke.is_none() {
            app.write_test_state();
        }
        wake.active(
            !crate::platform::suspended()
                && (smoke.is_some()
                    || app.redraw
                    || app.animation.is_some()
                    || app.pending.is_some()
                    || app.saves.pending > 0
                    || app.close.waiting()
                    || is_mouse_button_down(MouseButton::Left)
                    || !get_keys_down().is_empty()
                    || app.name_keyboard
                    || app.audio.needs_tick(&app.settings, get_time())),
        );
        next_frame().await;
        // Keep input and async workers polling while reducing static-screen GPU wakeups.
        if !cfg!(any(target_os = "windows", target_os = "android"))
            && smoke.is_none()
            && crate::platform::observation_path().is_none()
            && app.animation.is_none()
            && app.pending.is_none()
            && !app.close.waiting()
            && !is_mouse_button_down(MouseButton::Left)
            && get_keys_down().is_empty()
            && !app.name_keyboard
        {
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
        if app.name_keyboard && !matches!(app.modal, Some(Modal::Rename(..))) {
            macroquad::miniquad::window::show_keyboard(false);
            app.name_keyboard = false;
        }
    }
}
