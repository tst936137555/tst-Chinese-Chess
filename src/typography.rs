//! Startup glyph preparation, bounded density banks, and safe atlas expansion.
use macroquad::prelude::*;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
};
struct Bank {
    density: u16,
    font: Font,
    cached: HashSet<(char, u16)>,
}
type MeasurementCache = HashMap<(u16, u16, u32, String), [f32; 3]>;
pub struct UiFont {
    bytes: Vec<u8>,
    banks: RefCell<VecDeque<Bank>>,
    pixel_density: Cell<f32>,
    supported: HashSet<char>,
    measurements: RefCell<MeasurementCache>,
}
impl UiFont {
    /// Preserve the original name in storage; spell unsupported glyphs visibly on screen.
    pub fn display_text<'a>(&self, text: &'a str) -> std::borrow::Cow<'a, str> {
        fallback_text(text, |c| self.supported.contains(&c))
    }
    pub fn load(bytes: &[u8]) -> Result<Self, macroquad::Error> {
        let face = ttf_parser::Face::parse(bytes, 0).expect("内置字体损坏");
        let mut supported = HashSet::new();
        if let Some(cmap) = face.tables().cmap {
            for table in cmap.subtables.into_iter().filter(|t| t.is_unicode()) {
                table.codepoints(|code| {
                    if let Some(c) = char::from_u32(code)
                        && face.glyph_index(c).is_some()
                    {
                        supported.insert(c);
                    }
                });
            }
        }
        let density = screen_dpi_scale();
        let bank = Bank {
            density: (density * 2.).ceil() as u16,
            font: load_ttf_font_from_bytes(bytes)?,
            cached: HashSet::new(),
        };
        Ok(Self {
            bytes: bytes.to_vec(),
            banks: RefCell::new(VecDeque::from([bank])),
            pixel_density: Cell::new(density),
            supported,
            measurements: RefCell::new(HashMap::new()),
        })
    }
    pub fn set_pixel_density(&self, pixels_per_unit: f32) {
        self.pixel_density
            .set((pixels_per_unit.max(0.5) * 2.).ceil() / 2.);
    }
    fn raster(&self, size: f32) -> (u16, f32, u16) {
        let logical = f32::from(size as u16);
        let dpi = screen_dpi_scale();
        let raster = (logical * self.pixel_density.get() / dpi)
            .ceil()
            .clamp(1., 512.) as u16;
        (
            raster,
            logical / f32::from(raster),
            (f32::from(raster) * dpi).ceil() as u16,
        )
    }
    pub fn raster_key(&self, size: f32) -> u16 {
        self.raster(size).2
    }
    pub fn prepare(&self, text: &str, size: f32) {
        let (_, _, physical) = self.raster(size);
        let density = (self.pixel_density.get() * 2.).ceil() as u16;
        let mut banks = self.banks.borrow_mut();
        if let Some(index) = banks.iter().position(|b| b.density == density) {
            banks.swap(0, index);
        } else {
            // Submit text before evicting its atlas. Keep only current/previous density,
            // so resizing or alternating portrait/landscape cannot grow one giant texture.
            unsafe {
                macroquad::window::get_internal_gl().flush();
            }
            if banks.len() >= 2 {
                banks.pop_back();
            }
            let font = load_ttf_font_from_bytes(&self.bytes).expect("内置字体损坏");
            banks.push_front(Bank {
                density,
                font,
                cached: HashSet::new(),
            });
        }
        let bank = banks.front_mut().unwrap();
        if text.chars().all(|c| bank.cached.contains(&(c, physical))) {
            return;
        }
        let missing: HashSet<_> = text
            .chars()
            .filter(|c| !bank.cached.contains(&(*c, physical)))
            .collect();
        if missing.is_empty() {
            return;
        }
        unsafe {
            macroquad::window::get_internal_gl().flush();
        }
        let mut chars: Vec<_> = missing.into_iter().collect();
        chars.sort_unstable();
        bank.font.populate_font_cache(&chars, physical);
        bank.cached.extend(chars.into_iter().map(|c| (c, physical)));
    }
    pub fn measure(&self, text: &str, size: f32) -> TextDimensions {
        let (raster, scale, physical) = self.raster(size);
        let key = (
            size as u16,
            physical,
            screen_dpi_scale().to_bits(),
            text.to_owned(),
        );
        if let Some(&[width, height, offset_y]) = self.measurements.borrow().get(&key) {
            return TextDimensions {
                width,
                height,
                offset_y,
            };
        }
        let display = self.display_text(text);
        let text = display.as_ref();
        self.prepare(text, size);
        let measured = measure_text(
            text,
            Some(&self.banks.borrow().front().unwrap().font),
            raster,
            scale,
        );
        let mut measurements = self.measurements.borrow_mut();
        if measurements.len() >= 1024 {
            measurements.clear();
        }
        if text.len() <= 4096 {
            measurements.insert(key, [measured.width, measured.height, measured.offset_y]);
        }
        measured
    }
    pub fn draw(&self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        let display = self.display_text(text);
        let text = display.as_ref();
        self.prepare(text, size);
        let (raster, scale, _) = self.raster(size);
        draw_text_ex(
            text,
            x,
            y,
            TextParams {
                font: Some(&self.banks.borrow().front().unwrap().font),
                font_size: raster,
                font_scale: scale,
                color,
                ..Default::default()
            },
        );
    }
}

/// O(log n) measurements, always truncating at a UTF-8 character boundary.
pub fn ellipsize(text: &str, max_width: f32, measure: impl Fn(&str) -> f32) -> String {
    if measure(text) <= max_width {
        return text.to_owned();
    }
    if measure("…") > max_width {
        return String::new();
    }
    let boundaries: Vec<_> = text
        .char_indices()
        .map(|(i, _)| i)
        .chain([text.len()])
        .collect();
    let (mut low, mut high) = (0, boundaries.len() - 1);
    while low < high {
        let middle = (low + high).div_ceil(2);
        if measure(&format!("{}…", &text[..boundaries[middle]])) <= max_width {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    format!("{}…", &text[..boundaries[low]])
}

/// Wrap without dropping UTF-8 characters. The caller reserves the line height.
pub fn wrap_text(text: &str, max_width: f32, measure: impl Fn(&str) -> f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for ch in paragraph.chars() {
            let mut next = line.clone();
            next.push(ch);
            if !line.is_empty() && measure(&next) > max_width {
                lines.push(std::mem::take(&mut line));
            }
            line.push(ch);
        }
        lines.push(line);
    }
    lines
}

pub fn fallback_text(text: &str, supported: impl Fn(char) -> bool) -> std::borrow::Cow<'_, str> {
    if text.chars().all(|c| c.is_control() || supported(c)) {
        return std::borrow::Cow::Borrowed(text);
    }
    std::borrow::Cow::Owned(
        text.chars()
            .map(|c| {
                if c.is_control() || supported(c) {
                    c.to_string()
                } else {
                    format!("[U+{:04X}]", c as u32)
                }
            })
            .collect(),
    )
}
