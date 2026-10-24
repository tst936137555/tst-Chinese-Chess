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
