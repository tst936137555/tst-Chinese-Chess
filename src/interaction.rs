//! Coordinate and hit-area rules shared by pointer and keyboard interaction.
use crate::rules::Square;
pub struct Scrollbar {
    pub thumb_offset: f32,
    pub thumb_height: f32,
    pub max_offset: f32,
    travel: f32,
}
impl Scrollbar {
    pub fn new(view: f32, content: f32, offset: f32) -> Self {
        let view = view.max(1.);
        let content = content.max(view);
        let max_offset = content - view;
        let thumb_height = (view * view / content).max(28_f32.min(view));
        let travel = view - thumb_height;
        Self {
            thumb_offset: if max_offset > 0. {
                offset.clamp(0., max_offset) / max_offset * travel
            } else {
                0.
            },
            thumb_height,
            max_offset,
            travel,
        }
    }
    pub fn drag_to(&self, pointer_in_track: f32, grab: f32) -> f32 {
        if self.travel <= 0. {
            0.
        } else {
            ((pointer_in_track - grab) / self.travel).clamp(0., 1.) * self.max_offset
        }
    }
}
pub fn map_point(point: [f32; 2], viewport: [f32; 4], canvas: [f32; 2]) -> [f32; 2] {
    [
        (point[0] - viewport[0]) * canvas[0] / viewport[2],
        (point[1] - viewport[1]) * canvas[1] / viewport[3],
    ]
}
pub fn touch_area(rect: [f32; 4], units_per_pixel: f32) -> [f32; 4] {
    let [x, y, w, h] = rect;
    let width = w.max(44. * units_per_pixel);
    let height = h.max(44. * units_per_pixel);
    [x + (w - width) / 2., y + (h - height) / 2., width, height]
}
pub fn board_hit(point: [f32; 2], origin: [f32; 2], cell: f32, flip: bool) -> Option<Square> {
    if !point.iter().chain(origin.iter()).all(|v| v.is_finite()) || !cell.is_finite() || cell <= 0.
    {
        return None;
    }
    let f = (point[0] - origin[0]) / cell;
    let r = (point[1] - origin[1]) / cell;
    if !(-0.5..8.5).contains(&f) || !(-0.5..9.5).contains(&r) {
        return None;
    }
    let (f, r) = (f.round() as i8, r.round() as i8);
    Square::new(if flip { 8 - f } else { f }, if flip { 9 - r } else { r })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scrollbar_drag_reaches_both_ends_and_handles_short_content() {
        let bar = Scrollbar::new(400., 1600., 600.);
        assert_eq!(bar.thumb_height, 100.);
        assert_eq!(bar.thumb_offset, 150.);
        assert_eq!(bar.drag_to(200., 50.), 600.);
        assert_eq!(bar.drag_to(-50., 50.), 0.);
        assert_eq!(bar.drag_to(500., 50.), 1200.);
        let short = Scrollbar::new(400., 80., 99.);
        assert_eq!(short.thumb_height, 400.);
        assert_eq!(short.drag_to(100., 10.), 0.);
        assert_eq!(Scrollbar::new(30., 10000., 0.).thumb_height, 28.);
    }
    #[test]
    fn hit_targets_keep_viewport_size_and_center() {
        for scale in [0.5, 0.8125, 1., 1.5, 2.] {
            let r = touch_area([10., 20., 40., 42.], scale);
            assert!(r[2] / scale >= 43.99 && r[3] / scale >= 43.99);
            assert!((r[0] + r[2] / 2. - 30.).abs() < 0.001);
        }
    }
    #[test]
    fn pointer_mapping_handles_safe_area_and_resize() {
        assert_eq!(
            map_point([500., 200.], [100., 50., 800., 400.], [1000., 500.]),
            [500., 187.5]
        );
    }
    #[test]
    fn nearest_cell_corners_flipping_and_outside() {
        let square = board_hit([90.4, 90.4], [100., 100.], 20., false).unwrap();
        assert_eq!((square.file(), square.rank()), (0, 0));
        let square = board_hit([100., 100.], [100., 100.], 20., true).unwrap();
        assert_eq!((square.file(), square.rank()), (8, 9));
        assert!(board_hit([89., 100.], [100., 100.], 20., false).is_none());
        assert!(board_hit([f32::NAN, 100.], [100., 100.], 20., false).is_none());
    }
}
