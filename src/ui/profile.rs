//! Fixed-window profiling, isolated from smoke screenshots and real user data.
use super::*;
pub(super) struct Profile {
    root: std::path::PathBuf,
    scene: String,
    duration: f64,
    started: Option<f64>,
    frames: u64,
    draws: u64,
}
impl Profile {
    pub(super) fn from_args() -> Option<Self> {
        let args: Vec<_> = std::env::args().collect();
        let arg = |name| {
            args.iter()
                .position(|a| a == name)
                .and_then(|i| args.get(i + 1))
        };
        let root = std::path::PathBuf::from(arg("--profile-ui")?);
        std::fs::create_dir_all(&root).expect("profile output directory");
        Some(Self {
            root,
            scene: arg("--profile-scene")
                .cloned()
                .unwrap_or_else(|| "home".into()),
            duration: arg("--profile-seconds")
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(10.)
                .clamp(2., 120.),
            started: None,
            frames: 0,
            draws: 0,
        })
    }
    pub(super) fn state_path(&self) -> std::path::PathBuf {
        self.root.join("data/state.json")
    }
    pub(super) fn setup(&mut self, app: &mut App) {
        if self.started.is_some() {
            return;
        }
        match self.scene.as_str() {
            "review-moves" | "review-moves-portrait" | "review-moves-bottom" | "compact-review" => {
                let portrait = self.scene.ends_with("portrait");
                let compact = self.scene == "compact-review";
                request_new_screen_size(
                    if portrait {
                        390.
                    } else if compact {
                        844.
                    } else {
                        1100.
                    },
                    if portrait {
                        844.
                    } else if compact {
                        320.
                    } else {
                        800.
                    },
                );
                let mut game = Game::default();
                for i in 0..64 {
                    let moves = game.board.legal_moves();
                    if game.outcome != Outcome::Playing || moves.is_empty() {
                        break;
                    }
                    game.play(moves[(i * 17 + 9) % moves.len()]).unwrap();
                }
                let mut review = Review::new(game, true);
                review.scores = (0..review.scores.len())
                    .map(|i| {
                        Some(Score::Cp(if i % 2 == 0 {
                            0
                        } else {
                            -[0, 50, 150, 350, 700][i / 2 % 5]
                        }))
                    })
                    .collect();
                review.analyzing = false;
                review.cache_loading = false;
                review.attempted.fill(true);
                app.review = Some(review);
                app.screen = Screen::Review;
                app.open_modal(Modal::ReviewMoves);
                if self.scene.ends_with("bottom") {
                    app.modal_scroll = f32::MAX;
                }
            }
            "review-tactics" | "review-tactics-portrait" => {
                let portrait = self.scene.ends_with("portrait");
                request_new_screen_size(
                    if portrait { 390. } else { 1100. },
                    if portrait { 844. } else { 800. },
                );
                let mut game = Game::default();
                for mv in ["h2e2", "h7e7", "b0c2", "b9c7", "a3a4", "a6a5"] {
                    game.play(mv.parse().unwrap()).unwrap();
                }
                let mut review = Review::new(game, true);
                review.scores = [
                    Score::Cp(0),
                    Score::Cp(400),
                    Score::Cp(700),
                    Score::Cp(100),
                    Score::Cp(-300),
                    Score::Mate(-3),
                    Score::Mate(-2),
                ]
                .map(Some)
                .to_vec();
                review.ply = 5;
                review.analyzing = false;
                review.cache_loading = false;
                review.attempted.fill(true);
                app.review = Some(review);
                app.screen = Screen::Review;
            }
            "compact-drafts" | "compact-help" | "wide-setup" | "wide-audio" | "long-title" => {
                request_new_screen_size(
                    if self.scene == "long-title" {
                        390.
                    } else {
                        844.
                    },
                    if self.scene == "long-title" {
                        844.
                    } else {
                        320.
                    },
                );
                match self.scene.as_str() {
                    "compact-drafts" => {
                        for _ in 0..4 {
                            let mut r = Game::default().snapshot(true, 0, false);
                            r.title = "春日残局练习与中炮对局复盘记录完整名称显示检查".into();
                            app.data.drafts.push(r);
                        }
                        app.open_modal(Modal::Drafts(0));
                    }
                    "compact-help" => app.open_modal(Modal::Help),
                    "wide-setup" => app.open_modal(Modal::Difficulty),
                    "wide-audio" => app.open_modal(Modal::Audio),
                    "long-title" => {
                        let mut game = Game::default();
                        game.resign(Side::Red);
                        let mut r = game.snapshot(true, 0, false);
                        r.title = "春日残局练习与中炮对局复盘记录完整名称显示检查".into();
                        app.data.archives.push(r);
                        app.archive_dirty = true;
                        app.screen = Screen::Archives;
                    }
                    _ => unreachable!(),
                }
            }
            "game" | "4k" => {
                app.screen = Screen::Game;
                if self.scene == "4k" {
                    app.scene.smoke_resolution(3840, 2160);
                }
            }
            "review" => {
                if let Some(record) = app.data.archives.first() {
                    let mut review = Review::new(record.replay().expect("profile fixture"), true);
                    review.analyzing = false;
                    review.cache_loading = false;
                    for (i, score) in review.scores.iter_mut().enumerate() {
                        *score = Some(Score::Cp((i as i32 % 20 - 10) * 25));
                    }
                    app.review = Some(review);
                    app.screen = Screen::Review;
                }
            }
            _ => {}
        }
        app.redraw = true;
        self.started = Some(get_time());
    }
    pub(super) fn capture(&mut self, app: &App, painted: bool) -> bool {
        self.frames += 1;
        self.draws += u64::from(painted);
        let elapsed = get_time() - self.started.unwrap_or_else(get_time);
        if elapsed < self.duration {
            return false;
        }
        let result = serde_json::json!({ "scene":self.scene,"seconds":elapsed,"frames":self.frames,"sceneDraws":self.draws,
            "archives":app.data.archives.len(),"dpi":screen_dpi_scale(),"viewport":[screen_width(),screen_height()],
            "renderTarget":[app.scene.target.texture.width(),app.scene.target.texture.height()],"diagnostics":crate::diagnostics::snapshot(),
            "scope":"one isolated process; fixed viewport; CPU submit timings, not GPU timestamps or battery measurements" });
        std::fs::write(
            self.root.join("result.json"),
            serde_json::to_vec_pretty(&result).unwrap(),
        )
        .expect("profile result");
        get_screen_data().export_png(self.root.join("final.png").to_str().unwrap());
        true
    }
}
