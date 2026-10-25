//! Coarse visual baselines: tolerate rasterization noise while detecting layout/color regressions.
use macroquad::prelude::Image;
use std::path::Path;
fn cells(image: &Image, grid: usize) -> Vec<f64> {
    let mut sum = vec![0.; grid * grid * 3];
    let mut count = vec![0usize; grid * grid];
    for y in 0..image.height as usize {
        for x in 0..image.width as usize {
            let cell = (y * grid / image.height as usize) * grid + x * grid / image.width as usize;
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
fn crop(image: &Image, region: [f32; 4]) -> Image {
    let [x, y, w, h] = region;
    let left = (x * image.width as f32) as usize;
    let top = (y * image.height as f32) as usize;
    let width = (w * image.width as f32).round().max(1.) as u16;
    let height = (h * image.height as f32).round().max(1.) as u16;
    let mut result = Image::gen_image_color(width, height, macroquad::prelude::WHITE);
    for row in 0..height as usize {
        let start = ((top + row) * image.width as usize + left) * 4;
        result.bytes[row * width as usize * 4..(row + 1) * width as usize * 4]
            .copy_from_slice(&image.bytes[start..start + width as usize * 4]);
    }
    result
}
fn difference(a: &Image, b: &Image, grid: usize) -> f64 {
    let grid = grid
        .min(a.width as usize)
        .min(b.width as usize)
        .min(a.height as usize)
        .min(b.height as usize)
        .max(1);
    let a = cells(a, grid);
    let b = cells(b, grid);
    (a.iter().zip(b).map(|(a, b)| (a - b).powi(2)).sum::<f64>() / a.len() as f64).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thin_regions_compare_identically_across_dpi_without_empty_cells() {
        let a = Image::gen_image_color(20, 2, macroquad::prelude::WHITE);
        let b = Image::gen_image_color(40, 4, macroquad::prelude::WHITE);
        assert_eq!(difference(&a, &b, 64), 0.);
    }
    #[test]
    fn fractional_dpi_allows_only_one_physical_pixel_of_rounding() {
        for dpi in [1., 1.25, 1.5, 1.75, 2., 2.25] {
            let size = [
                (390.0_f64 * dpi).floor() as u16,
                (844.0_f64 * dpi).floor() as u16,
            ];
            assert!(viewport_matches(size, [390, 844], dpi));
            assert!(!viewport_matches([size[0] - 3, size[1]], [390, 844], dpi));
        }
    }
}
fn load(path: &Path) -> Result<Image, String> {
    let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Image::from_file_with_format(&b, None).map_err(|e| e.to_string())
}
fn viewport_matches(actual: [u16; 2], expected: [u16; 2], dpi: f64) -> bool {
    dpi.is_finite()
        && dpi > 0.
        && actual
            .into_iter()
            .zip(expected)
            .all(|(a, e)| (f64::from(a) - f64::from(e) * dpi).abs() <= 1.)
}
fn main() -> Result<(), String> {
    let root = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/ui-smoke".into());
    let metadata: serde_json::Value = serde_json::from_slice(
        &std::fs::read(Path::new(&root).join("result.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let dpi = metadata["deviceScale"].as_f64().unwrap_or(1.);
    for scene in [
        "01-red.png",
        "03-landscape-input.png",
        "10-audio.png",
        "18-desktop-review.png",
        "19-compact-setup.png",
        "22-help.png",
        "26-compact-draft-name.png",
    ] {
        let expected = load(&Path::new("tests/ui_baselines").join(scene))?;
        let actual = load(&Path::new(&root).join(scene))?;
        let dpi = metadata["viewports"][scene]["dpi"].as_f64().unwrap_or(dpi);
        if !viewport_matches(
            [actual.width, actual.height],
            [expected.width, expected.height],
            dpi,
        ) {
            return Err(format!("{scene}: viewport changed"));
        }
        let rms = difference(&actual, &expected, 32);
        if rms > 0.025 {
            return Err(format!(
                "{scene}: coarse RGB difference {rms:.4} exceeds 0.025"
            ));
        }
        println!("{scene}: coarse RGB RMS {rms:.4}");
        let regions = if scene == "01-red.png" {
            vec![
                ("board", [0.04, 0.28, 0.92, 0.48]),
                ("status", [0.06, 0.11, 0.88, 0.04]),
            ]
        } else if scene == "19-compact-setup.png" {
            vec![("difficulty choices", [0.12, 0.25, 0.76, 0.38])]
        } else {
            vec![("main content", [0.1, 0.15, 0.8, 0.7])]
        };
        for (name, region) in regions {
            let rms = difference(&crop(&actual, region), &crop(&expected, region), 64);
            if rms > 0.045 {
                return Err(format!(
                    "{scene}/{name}: local RGB RMS {rms:.4} exceeds 0.045"
                ));
            }
            println!("{scene}/{name}: local RGB RMS {rms:.4}");
        }
    }
    Ok(())
}
