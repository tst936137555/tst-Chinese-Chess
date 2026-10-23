//! Shared desktop/mobile UI with portrait and landscape layouts.
use crate::{
    engine::{Analysis, Engine, Score},
    game::{Game, LEVELS, Outcome, outcome_by_score},
    persistence::{Command, Response, Worker},
    review::CachedPosition,
    rules::{Board, Move, Piece, Side, Square},
    storage::{Data, Settings, Storage},
};
use macroquad::prelude::*;
mod board;
mod controller;
mod copy;
mod dialogs;
mod drawing;
mod layout;
mod loading;
mod screens;
mod smoke;

pub(crate) const PAPER: Color = Color::new(0.965, 0.957, 0.929, 1.);
const SURFACE: Color = Color::new(0.996, 0.988, 0.965, 1.);
const INK: Color = Color::new(0.14, 0.18, 0.16, 1.);
const RED_INK: Color = Color::new(0.66, 0.23, 0.18, 1.);
const WOOD: Color = Color::new(0.90, 0.80, 0.62, 1.);
const MUTED: Color = Color::new(0.35, 0.39, 0.34, 1.);
const GOLD: Color = Color::new(0.65, 0.51, 0.32, 1.);
const JADE: Color = Color::new(0.22, 0.42, 0.35, 1.);
const BORDER: Color = Color::new(0.85, 0.84, 0.79, 1.);

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Home,
    Game,
    Archives,
    Review,
}
enum Job {
    Move,
    Hint,
    Review(usize),
    End,
}
struct Pending {
    id: u64,
    job: Job,
}
use crate::review::ReviewSession as Review;
struct SavedGamePage {
    game: Game,
    user_red: bool,
    level: usize,
    review_game: bool,
}
enum PageState {
    Home,
    Archives(f32),
    Game(Box<SavedGamePage>),
    Review(Box<Review>),
}
enum Modal {
    Difficulty,
    Side(Option<Box<Game>>),
    Drafts(usize),
    SaveFailed,
    StorageRecovery,
    End(Option<crate::game::EvaluationEvidence>),
    ClearDrafts,
    Rename(String, crate::text_input::NameEdit),
    Delete(String),
    Clear,
    Help,
    About,
    License(usize),
    Audio,
}
struct Animation {
    mv: Move,
    piece: Piece,
    captured: Option<Piece>,
    started: f64,
}
struct App {
    font: crate::typography::UiFont,
    surfaces: drawing::Surfaces,
    scene: crate::rendering::AdaptiveScene,
    audio: crate::audio::AudioBank,
    screen: Screen,
    game: Game,
    settings: Settings,
    review_game: bool,
    io: Worker,
    saves: crate::session::SaveState,
    closing: bool,
    close_flushed: bool,
    force_exit: bool,
    focus: Option<Rect>,
    focus_index: usize,
    focus_count: usize,
    keyboard_active: bool,
    keyboard_nav: bool,
    name_keyboard: bool,
    rename_return: Option<usize>,
    board_cursor: Option<Square>,
    observed_controls: Vec<(String, Rect)>,
    observed_at: f64,
    data: Data,
    engine: Engine,
    pending: Option<Pending>,
    selected: Option<Square>,
    hints: Vec<Move>,
    notice: String,
    engine_failed: bool,
    review: Option<Review>,
    navigation: Vec<PageState>,
    width: f32,
    height: f32,
    viewport: Rect,
    click: Option<Vec2>,
    modal: Option<Modal>,
    scroll: f32,
    drag: Option<(Vec2, Option<String>)>,
    animation: Option<Animation>,
    ended_at: Option<f64>,
    end_ready: bool,
    license_texts: Vec<String>,
    modal_scroll: f32,
    modal_scroll_grab: Option<f32>,
    move_scroll: f32,
    dragged: bool,
    move_follow: bool,
    move_layout_key: (String, u64),
    move_labels: Vec<(String, Side, f32)>,
    move_offsets: Vec<f32>,
    move_total: f32,
    archive_order: Vec<usize>,
    archive_dirty: bool,
    archive_favorites: bool,
    legal_key: String,
    legal_cache: Vec<Move>,
    text_cache: std::collections::HashMap<String, Vec<String>>,
    recovery: crate::session::Recovery,
    pause_serial: u64,
    piece_cache: std::cell::RefCell<std::collections::VecDeque<(Piece, u16, RenderTarget)>>,
    board_surface_cache: std::cell::RefCell<Option<(bool, u16, RenderTarget)>>,
    accessible: Vec<crate::accessibility::Element>,
    accessibility_active: bool,
    accessibility_focus: Option<u32>,
}
pub async fn run() {
    let startup = std::time::Instant::now();
    let mut smoke = smoke::Smoke::from_args();
    if let Some(s) = &smoke {
        s.boot("font-load");
    }
    let font =
        crate::typography::UiFont::load(include_bytes!("../assets/fonts/XqKai-Medium-subset.ttf"))
            .expect("内置字体损坏");
    let loader = loading::LoadingScreen::new(
        smoke
            .as_ref()
            .map(|s| s.state_path().with_file_name("startup.png")),
    );
    loader.show(&font, 0.04).await;
    if let Some(s) = &smoke {
        s.boot("render-target-start");
    }
    crate::diagnostics::measure("first_frame_ms", startup.elapsed().as_secs_f64() * 1000.);
    let scene = crate::rendering::AdaptiveScene::new();
    if let Some(s) = &smoke {
        s.boot("render-target-ready");
    }
    let canvas_height = 900_f32.max(360. * screen_height() / screen_width());
    font.set_pixel_density(
        scene.target.texture.height() / canvas_height.min(screen_height().max(685.)),
    );
    let mut font_precache_ms = 0.;
    for (index, (text, size)) in copy::FONT_GROUPS.iter().enumerate() {
        // Help/about and long licenses are prepared when opened, after the first interaction.
        if index < 2 {
            continue;
        }
        if let Some(s) = &smoke {
            s.boot(&format!("font-group-{index}"));
        }
        let warm_started = std::time::Instant::now();
        font.prepare(text, *size);
        font_precache_ms += warm_started.elapsed().as_secs_f64() * 1000.;
        loader
            .show(
                &font,
                0.12 + 0.5 * (index + 1) as f32 / copy::FONT_GROUPS.len() as f32,
            )
            .await;
    }
    if let Some(s) = &smoke {
        s.boot("audio-start");
    }
    let audio = crate::audio::AudioBank::load().await;
    loader.show(&font, 0.68).await;
    let (mut path, mut notice) = match crate::platform::data_dir() {
        Ok(d) => (d.join("state.json"), String::new()),
        Err(e) => (std::path::PathBuf::from("state.json"), e),
    };
    #[cfg(any(target_os = "android", target_os = "ios"))]
    if let Some(parent) = path.parent() {
        let dest = parent.join("pikafish.nnue");
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../tool/engine_manifest.json"))
                .expect("asset manifest");
        let expected = manifest["pikafish"]["artifacts"]["nnue"]["sha256"]
            .as_str()
            .expect("NNUE hash")
            .to_owned();
        let (done, result) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            let result = crate::platform::prepare_nnue(&dest, &expected);
            crate::diagnostics::measure("nnue_prepare_ms", start.elapsed().as_secs_f64() * 1000.);
            let _ = done.send(result);
        });
        loop {
            match result.try_recv() {
                Ok(Ok(())) => break,
                Ok(Err(e)) => {
                    notice = format!("权重安装失败：{e}");
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    notice = "权重准备线程已退出".into();
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => loader.show(&font, 0.72).await,
            }
        }
    }
    let mut license_texts = vec![
        include_str!("../LICENSE").into(),
        String::new(),
        include_str!("../assets/fonts/OFL.txt").into(),
    ];
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        license_texts[1] = load_string("licenses/NNUE-License.md")
            .await
            .unwrap_or_else(|e| format!("许可证加载失败：{e}"));
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let mut paths = vec![std::path::PathBuf::from("assets/licenses/NNUE-License.md")];
        if let Ok(exe) = std::env::current_exe()
            && let Some(p) = exe.parent()
        {
            paths.push(p.join("assets/licenses/NNUE-License.md"));
        }
        license_texts[1] = paths
            .into_iter()
            .find_map(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_else(|| "请先运行引擎资产下载脚本获取 NNUE 许可证。".into());
    }
    loader.show(&font, 0.92).await;
    crate::diagnostics::measure("font_precache_ms", font_precache_ms);
    if let Some(s) = &smoke {
        path = s.state_path();
    }
    if let Some(s) = &smoke {
        s.boot("storage");
    }
    let store = Storage::new(path);
    let mut storage_failed = false;
    let mut data = match store.load() {
        Ok(d) => d,
        Err(e) => {
            storage_failed = true;
            notice = e;
            Data::default()
        }
    };
    let args: Vec<_> = std::env::args().collect();
    if let Some(i) = args.iter().position(|s| s == "--import")
        && let Some(p) = args.get(i + 1)
    {
        match store.import_legacy(std::path::Path::new(p)) {
            Ok((d, n)) => {
                data = d;
                notice = format!("已导入 {n} 局旧棋谱");
            }
            Err(e) => notice = e,
        }
    }
    data.settings.level = data.settings.level.min(4);
    let settings = data.settings.clone();
    let mut app = App {
        font,
        surfaces: drawing::Surfaces::new(),
        scene,
        audio,
        screen: Screen::Home,
        game: Game::default(),
        settings,
        review_game: false,
        io: Worker::new(store),
        saves: crate::session::SaveState::default(),
        closing: false,
        close_flushed: false,
        force_exit: false,
        focus: None,
        focus_index: 0,
        focus_count: 0,
        keyboard_active: true,
        keyboard_nav: false,
        name_keyboard: false,
        rename_return: None,
        board_cursor: None,
        observed_controls: Vec::new(),
        observed_at: 0.,
        data,
        engine: Engine::new(),
        pending: None,
        selected: None,
        hints: vec![],
        notice,
        engine_failed: false,
        review: None,
        navigation: Vec::new(),
        width: 640.,
        height: 900.,
        viewport: Rect::new(0., 0., screen_width(), screen_height()),
        click: None,
        modal: storage_failed.then_some(Modal::StorageRecovery),
        scroll: 0.,
        drag: None,
        animation: None,
        ended_at: None,
        end_ready: false,
        license_texts,
        modal_scroll: 0.,
        modal_scroll_grab: None,
        move_scroll: 0.,
        dragged: false,
        move_follow: true,
        move_layout_key: (String::new(), u64::MAX),
        move_labels: Vec::new(),
        move_offsets: Vec::new(),
        move_total: 0.,
        archive_order: Vec::new(),
        archive_dirty: true,
        archive_favorites: false,
        legal_key: String::new(),
        legal_cache: Vec::new(),
        text_cache: std::collections::HashMap::new(),
        recovery: crate::session::Recovery::default(),
        pause_serial: crate::platform::pause_serial(),
        piece_cache: std::cell::RefCell::new(std::collections::VecDeque::new()),
        board_surface_cache: std::cell::RefCell::new(None),
        accessible: Vec::new(),
        accessibility_active: true,
        accessibility_focus: None,
    };
    loader.show(&app.font, 1.).await;
    drop(loader);
    if let Some(s) = &smoke {
        s.boot("interactive");
    }
    prevent_quit();
    crate::diagnostics::measure("interactive_ms", startup.elapsed().as_secs_f64() * 1000.);
    let subscriber = macroquad::input::utils::register_input_subscriber();
    let mut lifecycle = Lifecycle;
    let mut idle_ms = 0_f32;
    loop {
        let frame_started = std::time::Instant::now();
        macroquad::input::utils::repeat_all_miniquad_input(&mut lifecycle, subscriber);
        if let Some(s) = &mut smoke {
            s.setup(&mut app);
        }
        app.handle_lifecycle();
        app.poll();
        if is_quit_requested() && !app.closing {
            app.save_review_cache();
            app.cancel();
            app.closing = true;
            app.close_flushed = false;
            app.notice = "正在保存后退出...".into();
            app.enqueue(Command::Flush);
        }
        if app.force_exit
            || (app.closing
                && app.close_flushed
                && app.saves.pending == 0
                && app.saves.failed.is_empty())
        {
            break;
        }
        if app.closing && !app.saves.failed.is_empty() {
            app.closing = false;
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
        {
            app.move_layout_key.1 = u64::MAX;
            app.drag = None;
            app.dragged = false;
            app.click = None;
        }
        let ratio = app.viewport.w / app.viewport.h;
        let split = ratio >= 1.35
            && matches!(app.screen, Screen::Game | Screen::Review)
            && (app.screen == Screen::Review || app.game.outcome == Outcome::Playing);
        app.height = if split && app.viewport.h < 500. {
            480.
        } else if split {
            600.
        } else {
            900.
        };
        app.width = app.height * ratio;
        if app.width < 360. {
            app.width = 360.;
            app.height = 360. / ratio;
        }
        app.scene.resize();
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
        if app.closing {
            app.click = None;
        }
        if let Some(id) = crate::accessibility::take_action()
            && let Some(element) = app.accessible.iter().find(|e| e.id == id && e.actionable)
        {
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
            app.click = Some(vec2(point[0], point[1]));
        }
        if app
            .animation
            .as_ref()
            .is_some_and(|a| get_time() - a.started >= 0.26)
        {
            app.animation = None;
        }
        if let Some(s) = &smoke {
            s.input(&mut app);
        }
        app.focus_count = 0;
        app.observed_controls.clear();
        app.accessible.clear();
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
        if app.screen == Screen::Game && app.game.outcome != Outcome::Playing && app.modal.is_none()
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
        if app.notice.is_empty() && app.saves.pending > 0 {
            app.fit_text(
                "正在后台保存...",
                12.,
                app.height - 7.,
                app.width - 24.,
                14.,
                MUTED,
            );
        }
        if !app.notice.is_empty() {
            draw_rectangle(0., app.height - 25., app.width, 25., PAPER);
            app.fit_text(
                &app.notice,
                12.,
                app.height - 7.,
                app.width - 24.,
                14.,
                RED_INK,
            );
        }
        if smoke.is_none()
            && !app.closing
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
        app.scene.present();
        let summary = match &app.modal {
            Some(Modal::Rename(_, edit)) => format!(
                "名称编辑：{}。已选中 {} 个字",
                edit.text,
                edit.selection().len()
            ),
            Some(_) => "中国象棋，对话框".into(),
            None => match app.screen {
                Screen::Game => format!(
                    "{}，{}行棋，{}",
                    app.game.outcome.label(),
                    app.game.board.turn.label(),
                    match app.saves.status(&app.game.id) {
                        crate::session::SaveStatus::Failed => "保存失败，可处理保存",
                        crate::session::SaveStatus::Pending => "正在保存",
                        _ => "已保存",
                    }
                ),
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
            app.enqueue(Command::Settings(app.settings.clone()));
        }
        if (is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl))
            && is_key_pressed(KeyCode::Q)
        {
            macroquad::miniquad::window::request_quit();
        }
        crate::diagnostics::measure("frame_ms", f64::from(get_frame_time()) * 1000.);
        let work_ms = frame_started.elapsed().as_secs_f32() * 1000.;
        crate::diagnostics::measure("frame_work_ms", f64::from(work_ms));
        if smoke.is_none() && !crate::platform::suspended() {
            app.scene.budget.observe(
                work_ms.max(get_frame_time() * 1000. - idle_ms),
                get_frame_time(),
            );
        }
        if crate::platform::smoke_output().is_none() && smoke.is_none() {
            app.write_test_state();
        }
        next_frame().await;
        idle_ms = 0.;
        // Keep input and async workers polling while reducing static-screen GPU wakeups.
        if smoke.is_none()
            && crate::platform::observation_path().is_none()
            && app.animation.is_none()
            && app.pending.is_none()
            && !app.closing
            && !is_mouse_button_down(MouseButton::Left)
            && get_keys_down().is_empty()
            && !app.name_keyboard
        {
            let idle_started = std::time::Instant::now();
            std::thread::sleep(std::time::Duration::from_millis(16));
            idle_ms = idle_started.elapsed().as_secs_f32() * 1000.;
        }
        if app.name_keyboard && !matches!(app.modal, Some(Modal::Rename(..))) {
            macroquad::miniquad::window::show_keyboard(false);
            app.name_keyboard = false;
        }
    }
}
impl App {
    fn semantic(&mut self, label: String, rect: Rect, actionable: bool) {
        if !self.accessibility_active || label.is_empty() {
            return;
        }
        let unit = self.width / self.viewport.w;
        self.accessible.push(crate::accessibility::Element::new(
            label,
            [
                self.viewport.x + rect.x / unit,
                self.viewport.y + rect.y / unit,
                rect.w / unit,
                rect.h / unit,
            ],
            actionable,
        ));
    }
    fn content(&self) -> (f32, f32) {
        if self.landscape() && matches!(self.screen, Screen::Game | Screen::Review) {
            let x = self.width * 0.56 + 20.;
            return (x, (self.width - x - 24.).min(680.));
        }
        let w = self.width.min(680.) - 32.;
        ((self.width - w) / 2., w)
    }
    fn open_modal(&mut self, m: Modal) {
        self.accessible.clear();
        if self.name_keyboard {
            crate::platform::name_editor("", 0, 0, [0.; 4], false);
            macroquad::miniquad::window::show_keyboard(false);
            self.name_keyboard = false;
        }
        if matches!(m, Modal::Rename(..)) {
            self.rename_return = match &self.modal {
                Some(Modal::Drafts(page)) => Some(*page),
                _ => None,
            };
            while get_char_pressed().is_some() {}
            self.name_keyboard = true;
            macroquad::miniquad::window::set_ime_enabled(true);
            #[cfg(not(target_os = "ios"))]
            macroquad::miniquad::window::show_keyboard(true);
        }
        self.modal = Some(m);
        self.modal_scroll = 0.;
        self.modal_scroll_grab = None;
    }
    fn finish_rename(&mut self) {
        self.accessible.clear();
        let _ = crate::text_input::take_native_edit();
        let _ = crate::text_input::take_native_done();
        crate::platform::name_editor("", 0, 0, [0.; 4], false);
        macroquad::miniquad::window::set_ime_enabled(false);
        macroquad::miniquad::window::show_keyboard(false);
        self.name_keyboard = false;
        self.modal = self.rename_return.take().map(Modal::Drafts);
    }
    fn back(&mut self) {
        self.accessible.clear();
        self.cancel();
        self.engine_failed = false;
        self.notice.clear();
        self.modal = None;
        self.animation = None;
        self.save_review_cache();
        self.review = None;
        match self.navigation.pop().unwrap_or(PageState::Home) {
            PageState::Home => self.screen = Screen::Home,
            PageState::Archives(scroll) => {
                self.scroll = scroll;
                self.screen = Screen::Archives;
            }
            PageState::Game(page) => {
                self.game = page.game;
                self.settings.user_red = page.user_red;
                self.settings.level = page.level;
                self.review_game = page.review_game;
                self.end_ready = true;
                self.reset_move_view();
                self.screen = Screen::Game;
            }
            PageState::Review(mut review) => {
                self.settings.user_red = review.user_red;
                review.cache_loading = true;
                self.review = Some(*review);
                self.screen = Screen::Review;
                self.load_review_cache();
            }
        }
        self.ended_at = None;
    }
    fn stash_page(&mut self) -> PageState {
        match self.screen {
            Screen::Home => PageState::Home,
            Screen::Archives => PageState::Archives(self.scroll),
            Screen::Game => PageState::Game(Box::new(SavedGamePage {
                game: self.game.clone(),
                user_red: self.settings.user_red,
                level: self.settings.level,
                review_game: self.review_game,
            })),
            Screen::Review => {
                PageState::Review(Box::new(self.review.take().expect("review page state")))
            }
        }
    }
    fn start_game(&mut self, board: Option<Game>, red: bool) {
        self.accessible.clear();
        if self.data.drafts.len() >= 50 && self.data.active.is_some() {
            self.notice = "对局已满，请先删除旧对局后开始新局".into();
            self.open_modal(Modal::Drafts(0));
            return;
        }
        if !self.saves.failed.is_empty() {
            self.notice = "请先重试保存，当前对局仍保留".into();
            self.open_modal(Modal::SaveFailed);
            return;
        }
        if self.screen != Screen::Game {
            let page = self.stash_page();
            self.navigation.push(page);
        }
        self.cancel();
        self.game = board.clone().unwrap_or_default();
        self.review_game = board.is_some();
        self.settings.user_red = red;
        self.engine_failed = false;
        self.notice.clear();
        self.animation = None;
        self.ended_at = None;
        self.end_ready = false;
        self.modal = None;
        self.reset_move_view();
        self.persist();
        self.screen = Screen::Game;
    }
    fn resume(&mut self) {
        if !self.data.drafts.is_empty() {
            self.open_modal(Modal::Drafts(0));
        } else if let Some(r) = self.data.active.clone() {
            self.resume_record(r);
        }
    }
    fn resume_record(&mut self, record: crate::game::Record) {
        self.accessible.clear();
        let record = self
            .saves
            .unsaved(&record.id)
            .filter(|failed| !record.acknowledges(failed))
            .cloned()
            .unwrap_or(record);
        match record.replay() {
            Ok(game) => {
                self.cancel();
                self.navigation.clear();
                self.navigation.push(PageState::Home);
                self.settings.user_red = record.user_red;
                self.settings.level = LEVELS
                    .iter()
                    .position(|l| l.name == record.level_name)
                    .unwrap_or(0);
                self.review_game = record.mode == "review";
                self.game = game;
                self.enqueue(Command::Activate(record.id));
                self.engine_failed = false;
                self.ended_at = None;
                self.end_ready = false;
                self.modal = None;
                self.reset_move_view();
                self.screen = Screen::Game;
            }
            Err(e) => self.notice = format!("续局无效：{e}"),
        }
    }
    fn open_review(&mut self, game: Game, _origin: Screen) {
        self.accessible.clear();
        let page = self.stash_page();
        self.navigation.push(page);
        self.cancel();
        self.review = Some(Review::new(game, self.settings.user_red));
        self.engine_failed = false;
        self.screen = Screen::Review;
        self.load_review_cache();
        self.ended_at = None;
    }
    fn change_level(&mut self, delta: isize) {
        self.settings.level = self.settings.level.saturating_add_signed(delta).min(4);
        self.persist();
    }
}

struct Lifecycle;
impl macroquad::miniquad::EventHandler for Lifecycle {
    fn update(&mut self) {}
    fn draw(&mut self) {}
    fn window_minimized_event(&mut self) {
        crate::platform::set_suspended(true);
    }
    fn window_restored_event(&mut self) {
        crate::platform::set_suspended(false);
    }
}

// Fine curves stay smooth when the automatic render target reaches 2K/4K.
fn draw_circle(x: f32, y: f32, radius: f32, color: Color) {
    draw_poly(x, y, 96, radius, 0., color);
}
fn draw_circle_lines(x: f32, y: f32, radius: f32, thickness: f32, color: Color) {
    draw_poly_lines(x, y, 96, radius, 0., thickness, color);
}
