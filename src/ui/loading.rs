use super::*;
pub(super) struct LoadingScreen {
    icon: Texture2D,
    capture: Option<std::path::PathBuf>,
}
impl LoadingScreen {
    pub(super) fn new(capture: Option<std::path::PathBuf>) -> Self {
        let icon =
            Texture2D::from_file_with_format(include_bytes!("../../assets/icons/app.png"), None);
        icon.set_filter(FilterMode::Linear);
        Self { icon, capture }
    }
    pub(super) async fn show(&self, font: &crate::typography::UiFont, progress: f32) {
        set_default_camera();
        clear_background(PAPER);
        let center = vec2(screen_width() / 2., screen_height() / 2. - 20.);
        let icon_size = screen_width().min(screen_height()).min(600.) * 0.26;
        draw_texture_ex(
            &self.icon,
            center.x - icon_size / 2.,
            center.y - icon_size - 30.,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(icon_size, icon_size)),
                ..Default::default()
            },
        );
        let title = "中国象棋";
        let m = font.measure(title, 32.);
        font.draw(title, center.x - m.width / 2., center.y + 26., 32., INK);
        let motto = "静心对弈 · 步步有思";
        let m = font.measure(motto, 15.);
        font.draw(motto, center.x - m.width / 2., center.y + 61., 15., MUTED);
        let width = (screen_width() * 0.52).min(300.);
        let (x, y) = (center.x - width / 2., center.y + 106.);
        draw_rectangle(x, y, width, 4., BORDER);
        draw_rectangle(x, y, width * progress.clamp(0., 1.), 4., JADE);
        if (0.60..0.64).contains(&progress)
            && let Some(path) = &self.capture
        {
            get_screen_data().export_png(path.to_str().unwrap());
        }
        next_frame().await;
    }
}
