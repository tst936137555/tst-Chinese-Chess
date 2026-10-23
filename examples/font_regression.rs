//! Compare the first frame of newly rasterized Chinese text with its warm-cache frame.
use macroquad::prelude::*;
use tst_xiangqi::typography::UiFont;
#[path = "../src/ui/copy.rs"]
mod copy;
fn config() -> Conf {
    Conf {
        window_title: "Font atlas regression".into(),
        window_width: 640,
        window_height: 800,
        sample_count: 1,
        high_dpi: true,
        ..Default::default()
    }
}
#[macroquad::main(config)]
async fn main() {
    let args: Vec<_> = std::env::args().collect();
    let raw = args.iter().any(|s| s == "--raw");
    let root = std::path::PathBuf::from(args.get(1).expect("output directory"));
    std::fs::create_dir_all(&root).unwrap();
    let bytes = include_bytes!("../assets/fonts/XqKai-Medium-subset.ttf");
    let font = UiFont::load(bytes).unwrap();
    let control = load_ttf_font_from_bytes(bytes).unwrap();
    // Exercise real startup preparation on the protected path.
    if !raw {
        for (text, size) in copy::FONT_GROUPS.iter().skip(2) {
            font.prepare(text, *size);
        }
    }
    let mut first: Option<Image> = None;
    for frame in 0..2 {
        clear_background(Color::new(0.98, 0.97, 0.94, 1.));
        let mut y = 35.;
        for (source, size) in [
            (copy::HELP, 18.),
            (copy::ABOUT, 17.),
            (copy::COMMON, 22.),
            (copy::COMMON, 26.),
            (copy::HELP, 15.),
        ] {
            let chars: Vec<_> = source.chars().filter(|c| !c.is_control()).collect();
            for chunk in chars.chunks(30).take(5) {
                let text: String = chunk.iter().collect();
                if raw {
                    draw_text_ex(
                        &text,
                        16.,
                        y,
                        TextParams {
                            font: Some(&control),
                            font_size: size as u16,
                            color: DARKGRAY,
                            ..Default::default()
                        },
                    );
                } else {
                    font.draw(&text, 16., y, size, DARKGRAY);
                }
                y += 27.;
            }
        }
        let image = get_screen_data();
        image.export_png(root.join(format!("{frame}.png")).to_str().unwrap());
        if let Some(first) = &first {
            let rms = (image
                .bytes
                .iter()
                .zip(&first.bytes)
                .map(|(a, b)| ((*a as f64 - *b as f64) / 255.).powi(2))
                .sum::<f64>()
                / image.bytes.len() as f64)
                .sqrt();
            let result = serde_json::json!({"rawControl":raw,"firstVsWarmRms":rms,"size":[image.width,image.height]});
            std::fs::write(
                root.join("result.json"),
                serde_json::to_vec_pretty(&result).unwrap(),
            )
            .unwrap();
            if !raw {
                assert!(
                    rms < 0.001,
                    "new glyphs changed the first rendered frame: {rms}"
                );
            }
        } else {
            first = Some(image);
        }
        next_frame().await;
    }
}
