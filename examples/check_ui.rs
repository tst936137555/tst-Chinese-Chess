//! Coarse visual baselines: tolerate rasterization noise while detecting layout/color regressions.
use macroquad::prelude::Image;
use std::path::Path;
fn cells(image: &Image) -> Vec<f64> {
    let mut sum = vec![0.; 32 * 32 * 3];
    let mut count = vec![0usize; 32 * 32];
    for y in 0..image.height as usize {
        for x in 0..image.width as usize {
            let cell = (y * 32 / image.height as usize) * 32 + x * 32 / image.width as usize;
            count[cell] += 1;
            for channel in 0..3 {
                sum[cell * 3 + channel] +=
                    image.bytes[(y * image.width as usize + x) * 4 + channel] as f64 / 255.;
            }
        }
    }
    for (cell, n) in count.into_iter().enumerate() {
        for channel in 0..3 {
            sum[cell * 3 + channel] /= n.max(1) as f64;
        }
    }
    sum
}
fn load(path: &Path) -> Result<Image, String> {
    let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Image::from_file_with_format(&b, None).map_err(|e| e.to_string())
}
fn main() -> Result<(), String> {
    let root = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/ui-smoke".into());
    for scene in ["01-red.png", "19-compact-setup.png"] {
        let expected = load(&Path::new("tests/ui_baselines").join(scene))?;
        let actual = load(&Path::new(&root).join(scene))?;
        if (actual.width, actual.height) != (expected.width, expected.height) {
            return Err(format!("{scene}: viewport changed"));
        }
        let a = cells(&actual);
        let b = cells(&expected);
        let rms =
            (a.iter().zip(b).map(|(a, b)| (a - b).powi(2)).sum::<f64>() / a.len() as f64).sqrt();
        if rms > 0.025 {
            return Err(format!(
                "{scene}: coarse RGB difference {rms:.4} exceeds 0.025"
            ));
        }
        println!("{scene}: coarse RGB RMS {rms:.4}");
    }
    Ok(())
}
