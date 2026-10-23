pub mod accessibility;
pub mod audio;
pub mod calibration;
pub mod diagnostics;
pub mod engine;
pub mod game;
pub mod interaction;
pub mod notation;
pub mod persistence;
pub mod platform;
pub mod rendering;
pub mod review;
pub mod rules;
pub mod self_test;
pub mod session;
pub mod storage;
pub mod text_input;
pub mod typography;
extern crate self as tst_xiangqi;

pub mod ui;

pub fn launch() {
    #[cfg(target_os = "ios")]
    {
        unsafe extern "C" {
            fn xq_configure_audio_session();
        }
        // Configured before macroquad opens its audio queue. No microphone permission.
        unsafe {
            xq_configure_audio_session();
        }
    }
    macroquad::Window::from_config(
        macroquad::prelude::Conf {
            window_title: "tst 自用象棋 · Rust".into(),
            window_width: 1100,
            window_height: 800,
            high_dpi: true,
            sample_count: 4,
            icon: app_icon(),
            ..Default::default()
        },
        ui::run(),
    );
}

fn app_icon() -> Option<macroquad::miniquad::conf::Icon> {
    use macroquad::prelude::Image;
    fn pixels<const N: usize>(bytes: &[u8]) -> Option<[u8; N]> {
        Image::from_file_with_format(bytes, None)
            .ok()?
            .bytes
            .try_into()
            .ok()
    }
    Some(macroquad::miniquad::conf::Icon {
        small: pixels(include_bytes!("../assets/icons/desktop/icon-16.png"))?,
        medium: pixels(include_bytes!("../assets/icons/desktop/icon-32.png"))?,
        big: pixels(include_bytes!("../assets/icons/desktop/icon-64.png"))?,
    })
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn quad_main() {
    launch();
}
