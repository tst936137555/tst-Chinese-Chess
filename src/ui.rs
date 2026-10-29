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
mod bootstrap;
mod controller;
mod copy;
mod dialogs;
mod drawing;
mod feedback;
mod layout;
mod loading;
mod navigation;
mod profile;
mod runtime;
mod screens;
mod smoke;
mod startup;

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
use crate::session::EngineTask as Job;
struct Pending {
    id: u64,
    job: Job,
    started: f64,
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
    ReviewMoves(usize),
    Search(crate::text_input::NameEdit),
    Backups(Vec<std::path::PathBuf>),
    ImportBackup(std::path::PathBuf),
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
    Variation(String),
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
    preferences: Settings,
    review_game: bool,
    io: Worker,
    saves: crate::session::SaveState,
    close: crate::session::CloseState,
    focus: Option<Rect>,
    focus_index: usize,
    focus_count: usize,
    keyboard_active: bool,
    keyboard_nav: bool,
    name_keyboard: bool,
    rename_return: Option<usize>,
    rename_revision: u64,
    delete_targets: Vec<(String, String)>,
    board_cursor: Option<Square>,
    observed_controls: Vec<(String, Rect)>,
    observed_at: f64,
    observed_pointer: [u64; 2],
    observed_release: serde_json::Value,
    data: Data,
    engine: Engine,
    pending: Option<Pending>,
    ready_move: Option<(Move, f64)>,
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
    archive_query: String,
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
    semantic_scope: String,
    semantic_counts: std::collections::HashMap<String, usize>,
    focus_request: Option<u32>,
    redraw: bool,
    loader: crate::replay::Loader,
    loading_record: Option<u64>,
    native_generation: u64,
}
pub use runtime::run;

impl App {
    fn semantic(&mut self, label: String, rect: Rect, actionable: bool) {
        let key = crate::accessibility::Element::label_identity(&label).to_owned();
        self.semantic_named(&key, label, rect, actionable);
    }
    fn semantic_named(&mut self, key: &str, label: String, rect: Rect, actionable: bool) {
        if !self.accessibility_active || label.is_empty() {
            return;
        }
        let unit = self.width / self.viewport.w;
        let key = format!("{}:{key}", self.semantic_scope);
        let occurrence = self.semantic_counts.entry(key.clone()).or_default();
        let identity = format!("{key}:{occurrence}");
        *occurrence += 1;
        self.accessible
            .push(crate::accessibility::Element::with_identity(
                &identity,
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
