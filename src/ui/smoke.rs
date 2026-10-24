//! Deterministic rendering/input smoke scenes. All writes use a separate output directory.
use super::*;
use std::path::PathBuf;
pub(super) struct Smoke {
    root: PathBuf,
    frame: usize,
    started: std::time::Instant,
    headless: Option<std::thread::JoinHandle<Result<(), String>>>,
}
impl Smoke {
    pub(super) fn from_args() -> Option<Self> {
        let args: Vec<_> = std::env::args().collect();
        let root = if let Some(i) = args.iter().position(|s| s == "--smoke-test") {
            PathBuf::from(
                args.get(i + 1)
                    .expect("--smoke-test requires output directory"),
            )
        } else {
            crate::platform::smoke_output()?
        };
        std::fs::create_dir_all(&root).expect("smoke output");
        Some(Self {
            root,
            frame: 0,
            started: std::time::Instant::now(),
            headless: None,
        })
    }
    pub(super) fn state_path(&self) -> PathBuf {
        self.root.join("state.json")
    }
    pub(super) fn boot(&self, stage: &str) {
        eprintln!("smoke startup: {stage}");
        let _ = std::fs::write(self.root.join("startup.txt"), stage);
    }
    pub(super) fn setup(&mut self, app: &mut App) {
        let stage = self.frame / 24;
        let phase = self.frame % 24;
        if phase != 0 {
            return;
        }
        self.boot(&format!("scene-{stage}/29"));
        app.cancel();
        app.modal = None;
        app.animation = None;
        app.engine_failed = false;
        app.notice.clear();
        match stage {
            0 => {
                let engine_present = cfg!(any(target_os = "android", target_os = "ios"))
                    || crate::platform::engine_path().is_ok();
                if engine_present && crate::platform::nnue_path().is_ok() {
                    self.headless = Some(std::thread::spawn(crate::self_test::run));
                }
                request_new_screen_size(390., 844.);
                app.screen = Screen::Home;
            }
            1 => {
                app.start_game(None, true);
                app.engine_failed = false;
            }
            2 => {
                app.settings.user_red = false;
                app.selected = Square::new(1, 2);
            }
            3 => {
                request_new_screen_size(844., 390.);
                app.settings.user_red = true;
                app.selected = None;
            }
            4 => {
                let game = app.game.clone();
                app.open_review(game, Screen::Game);
                let r = app.review.as_mut().unwrap();
                r.analyzing = false;
                r.cache_loading = false;
                app.pending = Some(Pending {
                    id: 9_000_001,
                    job: Job::Review(0),
                    started: get_time(),
                });
            }
            5 => {
                request_new_screen_size(390., 844.);
            }
            6 => {
                app.back();
                app.open_modal(Modal::End(None));
                app.engine_failed = true;
            }
            7 => {
                app.finish_resign();
                app.screen = Screen::Archives;
                app.archive_dirty = true;
            }
            8 => {
                app.screen = Screen::Home;
                let mut game = Game::default();
                game.play("h2e2".parse().unwrap()).unwrap();
                app.data.active = Some(game.snapshot(true, 2, false));
                app.data.drafts = (0..6)
                    .map(|i| {
                        let mut draft = game.clone();
                        draft.fork_identity();
                        let mut record = draft.snapshot(i % 2 == 0, i % 5, false);
                        record.title = format!("开局练习 {}", i + 1);
                        record
                    })
                    .collect();
                app.open_modal(Modal::Drafts(0));
            }
            9 => {
                app.screen = Screen::Home;
                app.open_modal(Modal::Difficulty);
            }
            10 => {
                app.open_modal(Modal::Audio);
            }
            11 => {
                request_new_screen_size(844., 390.);
                app.open_modal(Modal::Audio);
            }
            12 => {
                request_new_screen_size(390., 844.);
                app.open_modal(Modal::Side(Some(Box::default())));
            }
            13 => {
                request_new_screen_size(1100., 800.);
                app.screen = Screen::Home;
            }
            14 => {
                request_new_screen_size(768., 1024.);
                app.start_game(None, true);
            }
            15 => {
                request_new_screen_size(1440., 768.);
                app.engine_failed = true;
            }
            16 => {
                request_new_screen_size(390., 844.);
                app.game.resign(Side::Red);
            }
            17 => {
                let mut game = Game::default();
                for mv in ["h2e2", "h7e7", "b0c2", "b9c7", "a3a4", "a6a5"] {
                    game.play(mv.parse().unwrap()).unwrap();
                }
                app.open_review(game, Screen::Game);
                let r = app.review.as_mut().unwrap();
                r.analyzing = false;
                r.cache_loading = false;
                r.scores = [0, 20, 110, 70, -65, 160, 350]
                    .map(|cp| Some(Score::Cp(cp)))
                    .to_vec();
                r.depths.fill(18);
                for (i, entry) in r.game.history.iter().enumerate() {
                    r.bests[i] = Some(entry.mv);
                }
            }
            18 => {
                request_new_screen_size(1280., 800.);
            }
            19 => {
                request_new_screen_size(320., 568.);
                app.screen = Screen::Home;
                app.open_modal(Modal::Difficulty);
            }
            20 => {
                request_new_screen_size(390., 844.);
                app.screen = Screen::Archives;
                app.archive_favorites = true;
                app.archive_dirty = true;
            }
            21 => {
                app.archive_favorites = false;
                let mut game = Game::default();
                game.resign(Side::Black);
                let record = game.snapshot(true, 2, false);
                app.data.archives = (0..18)
                    .map(|i| {
                        let mut r = record.clone();
                        r.id = format!("smoke-{i}");
                        r.updated_at += i;
                        r
                    })
                    .collect();
                app.archive_dirty = true;
                app.scroll = 53.;
            }
            22 => {
                app.screen = Screen::Game;
                app.game = Game::default();
                app.open_modal(Modal::Help);
            }
            23 => {
                app.screen = Screen::Archives;
                app.open_modal(Modal::Delete("smoke-0".into()));
            }
            24 => {
                request_new_screen_size(390., 844.);
                app.notice = "此局已由另一窗口更新，当前棋盘仍保留。请重新载入或另存分支。".into();
                app.saves
                    .failed(Command::save(app.game.snapshot(true, 2, false), None));
                app.open_modal(Modal::SaveFailed);
            }
            25 => {
                app.saves.failed.clear(); // Remove the synthetic conflict before normal smoke shutdown.
                app.open_modal(Modal::About);
                app.modal_scroll = 80.;
            }
            26 => {
                // A short viewport also models the space above an on-screen keyboard.
                request_new_screen_size(844., 320.);
                let id = app.data.archives[0].id.clone();
                app.open_modal(Modal::Rename(id, "残局复盘".into()));
            }
            27 | 28 => {
                let (w, h) = if stage == 27 {
                    (2560, 1440)
                } else {
                    (3840, 2160)
                };
                let (w, h) = if screen_width() < screen_height() {
                    (h, w)
                } else {
                    (w, h)
                };
                app.scene.smoke_resolution(w, h);
                app.screen = Screen::Game;
                app.game = Game::default();
                app.engine_failed = true;
            }
            _ => {}
        }
    }
    pub(super) fn input(&self, app: &mut App) {
        let stage = self.frame / 24;
        let phase = self.frame % 24;
        if stage == 7 {
            if phase == 6 {
                let (x, w) = app.content();
                app.click = Some(vec2(x + w - 112., 319.));
            }
            if phase == 8 {
                assert!(
                    matches!(app.modal, Some(Modal::Rename(..))),
                    "archive rename opens"
                );
            }
            if phase == 10 || phase == 14 {
                let scale = app.height.min(app.viewport.h.max(340.)) / app.height;
                let w = (app.width * scale - 40.).min(480.);
                let x = (app.width * scale - w) / 2.;
                let y = (app.height * scale - 292.) / 2.;
                let point = if phase == 10 {
                    vec2(x + w / 2., y + 169.)
                } else {
                    vec2(x + w * 0.75 - 10., y + 242.)
                };
                app.click = Some(point / scale);
            }
            if phase == 16 {
                assert!(
                    app.modal.is_none() && app.screen == Screen::Archives,
                    "rename returns to archive list"
                );
            }
        }
        if stage == 9 {
            let scale = app.height.min(app.viewport.h.max(685.)) / app.height;
            let w = (app.width * scale - 40.).min(480.);
            let x = (app.width * scale - w) / 2.;
            let y = (app.height * scale - 605.) / 2.;
            if phase == 6 {
                app.click = Some(vec2(app.width * scale / 2., y + 110. + 4. * 59. + 25.) / scale);
            }
            if phase == 23 {
                app.click = Some(vec2(x + w * 0.75, y + 492.) / scale);
            }
        }
        if stage == 10 && phase == 6 {
            let scale = app.height.min(app.viewport.h.max(685.)) / app.height;
            let w = (app.width * scale - 40.).min(480.);
            let x = (app.width * scale - w) / 2.;
            let y = (app.height * scale - 605.) / 2.;
            app.click = Some(vec2(x + 108. + (w - 216.) * 0.75, y + 275.) / scale);
        }
        if stage == 4 && phase == 6 {
            app.review.as_mut().unwrap().ply = 1;
        }
        if stage == 4 && phase == 8 {
            let board = &app.review.as_ref().unwrap().game.start;
            let mv = board.legal_moves()[0];
            app.engine.smoke_reply(crate::engine::Reply {
                failure_kind: None,
                id: 9_000_001,
                result: Ok(Analysis {
                    best: mv,
                    variations: vec![crate::engine::Variation {
                        mv,
                        score: 15,
                        depth: 8,
                        index: 1,
                        evaluation: Score::Cp(15),
                    }],
                    elapsed_ms: 20,
                    requested_depth: 8,
                    budget_ms: 500,
                    assets: "controlled-smoke".into(),
                }),
            });
        }
        if stage == 14 && [6, 10, 14, 18].contains(&phase) {
            let (origin, cell) = app.board_rect();
            let square = if phase == 18 {
                Square::new(0, 5)
            } else {
                Square::new(7, 7)
            }
            .unwrap();
            app.click = Some(App::point(origin, cell, square, false));
        }
        if stage == 14 && phase == 12 {
            assert!(
                app.selected.is_none(),
                "tapping selected piece cancels selection"
            );
        }
        if stage == 17 {
            let (x, w) = app.content();
            let bw = (w - 24.) / 4.;
            if phase == 6 {
                app.click = Some(vec2(x + 3. * (bw + 8.) + bw / 2., app.height - 55.));
            }
            if phase == 8 {
                assert_eq!(app.review.as_ref().unwrap().ply, 6, "last move control");
            }
            if phase == 10 {
                app.click = Some(vec2(x + bw / 2., app.height - 55.));
            }
            if phase == 12 {
                assert_eq!(app.review.as_ref().unwrap().ply, 0, "first move control");
            }
            if phase == 14 {
                app.click = Some(vec2(x + 2. * (bw + 8.) + bw / 2., app.height - 55.));
            }
            if phase == 16 {
                assert_eq!(app.review.as_ref().unwrap().ply, 1, "next move control");
            }
            if phase == 18 {
                app.click = Some(vec2(x + 14. + (w - 28.) * 0.8, 735.));
            }
        }

        if self.frame / 24 == 6 && self.frame % 24 == 23 {
            let scale = app.height.min(app.viewport.h.max(540.)) / app.height;
            let y = (app.height * scale - 460.) / 2.;
            app.click = Some(vec2(app.width * scale / 2., y + 224.) / scale);
        }
        if self.frame / 24 == 3 {
            let phase = self.frame % 24;
            if phase == 6 || phase == 10 {
                let (origin, cell) = app.board_rect();
                let square = if phase == 6 {
                    Square::new(7, 7)
                } else {
                    Square::new(4, 7)
                }
                .unwrap();
                app.click = Some(App::point(origin, cell, square, false));
            }
        }
    }
    pub(super) fn capture(&mut self, app: &mut App) -> bool {
        let stage = self.frame / 24;
        let phase = self.frame % 24;
        if stage == 7 && phase == 5 && app.saves.pending > 0 {
            assert!(self.started.elapsed() < std::time::Duration::from_secs(60));
            return false;
        }
        if phase == 22 {
            if stage == 7 && app.saves.pending > 0 {
                assert!(
                    self.started.elapsed() < std::time::Duration::from_secs(60),
                    "archive acknowledgement timed out: {}",
                    app.notice
                );
                return false;
            }
            if stage == 3 && app.animation.is_some() {
                return false;
            }
            if stage == 3 {
                assert_eq!(
                    app.game.history.len(),
                    1,
                    "landscape board input must play h2e2"
                );
            }
            if stage == 4 {
                assert_eq!(
                    app.review.as_ref().unwrap().ply,
                    1,
                    "analysis must not move viewer cursor"
                );
                assert_eq!(
                    app.review.as_ref().unwrap().scores[0],
                    Some(Score::Cp(15)),
                    "controlled background result must be applied"
                );
            }
            if stage == 7 {
                assert!(
                    app.data
                        .archives
                        .iter()
                        .filter(|r| r.id == app.game.id)
                        .count()
                        == 1,
                    "resignation must be archived"
                );
                assert!(
                    app.data
                        .archives
                        .iter()
                        .any(|r| r.id == app.game.id && r.title == "残局练习"),
                    "archive rename is durable"
                );
            }
            if stage == 9 {
                assert_eq!(app.settings.level, 4, "difficulty selection");
            }
            if stage == 10 {
                assert!(
                    (app.settings.music_volume - 0.75).abs() < 0.01,
                    "sound slider tap"
                );
            }
            if stage == 14 {
                assert_eq!(
                    app.selected,
                    Square::new(7, 7),
                    "illegal target preserves selection"
                );
                assert!(app.game.history.is_empty());
            }
            if stage == 17 {
                assert_eq!(app.review.as_ref().unwrap().ply, 5, "chart selection");
            }
            if stage == 20 {
                assert!(app.archive_order.is_empty(), "favorites filter");
            }
            let name = [
                "home",
                "red",
                "black-selection",
                "landscape-input",
                "landscape-review",
                "portrait-review",
                "end-dialog",
                "archives",
                "drafts",
                "new-game",
                "audio",
                "landscape-audio",
                "branch-side",
                "desktop-home",
                "tablet-selection",
                "wide-engine-retry",
                "game-result",
                "review-navigation",
                "desktop-review",
                "compact-setup",
                "empty-favorites",
                "archive-scroll",
                "help",
                "delete-dialog",
                "save-failure",
                "about-scrollbar",
                "compact-draft-name",
                "2k-game",
                "4k-game",
            ][stage];
            let path = self.root.join(format!("{stage:02}-{name}.png"));
            let mut ids = std::collections::BTreeSet::new();
            for node in &app.accessible {
                assert!(
                    ids.insert(node.id),
                    "semantic identity collision in scene {stage}"
                );
                assert!(!node.label.is_empty() && node.rect.iter().all(|n| n.is_finite()));
                assert!(node.rect[2] > 0. && node.rect[3] > 0.);
                assert!(!node.disabled || (node.control && !node.actionable));
            }
            std::fs::write(
                self.root.join(format!("{stage:02}-semantics.json")),
                serde_json::to_vec_pretty(&app.accessible).unwrap(),
            )
            .unwrap();
            if stage >= 27 {
                // export_png already converts bottom-up GPU readback to upright rows.
                let frame = app.scene.target.texture.get_texture_data();
                frame.export_png(path.to_str().unwrap());
            } else {
                get_screen_data().export_png(path.to_str().unwrap());
            }
        }
        if stage == 9 && phase == 23 {
            assert!(
                app.screen == Screen::Game && !app.settings.user_red && app.modal.is_none(),
                "black-side start from setup"
            );
        }
        self.frame += 1;
        if self.frame == 29 * 24 {
            if self.headless.as_ref().is_some_and(|h| !h.is_finished()) {
                assert!(
                    self.started.elapsed() < std::time::Duration::from_secs(60),
                    "real-engine smoke timeout"
                );
                self.frame -= 1;
                return false;
            }
            let engine = if let Some(h) = self.headless.take() {
                h.join()
                    .expect("engine smoke thread")
                    .expect("real-engine installed smoke");
                "passed"
            } else {
                "assets unavailable"
            };
            let result = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"realEngine":engine,"deviceScale":screen_dpi_scale(),"scenes":29,"frames":self.frame,"elapsedMs":self.started.elapsed().as_millis(),"input":"internal hit-testing; OS input tested separately","archives":"resignation saved","cursor":"controlled asynchronous reply applied without moving cursor","renderTarget":[app.scene.target.texture.width(),app.scene.target.texture.height()],"performance":crate::diagnostics::snapshot()});
            std::fs::write(
                self.root.join("result.json"),
                serde_json::to_vec_pretty(&result).unwrap(),
            )
            .unwrap();
            return true;
        }
        false
    }
}
