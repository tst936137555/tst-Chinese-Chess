//! Startup glyph preparation, bounded density banks, and safe atlas expansion.
use macroquad::prelude::*;
use std::{
    cell::{Cell, RefCell},
    collections::{HashSet, VecDeque},
};
struct Bank {
    density: u16,
    font: Font,
    cached: HashSet<(char, u16)>,
}
pub struct UiFont {
    bytes: Vec<u8>,
    banks: RefCell<VecDeque<Bank>>,
    pixel_density: Cell<f32>,
}
impl UiFont {
    pub fn load(bytes: &[u8]) -> Result<Self, macroquad::Error> {
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
        self.prepare(text, size);
        let (raster, scale, _) = self.raster(size);
        measure_text(
            text,
            Some(&self.banks.borrow().front().unwrap().font),
            raster,
            scale,
        )
    }
    pub fn draw(&self, text: &str, x: f32, y: f32, size: f32, color: Color) {
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
