//! Aspect-preserving canvas with a separate landscape board and side panel.
pub(super) fn board_geometry(width: f32, height: f32, review: bool) -> (f32, f32, f32) {
    let landscape = height < 800. && width / height >= 1.35;
    let region_width = if landscape {
        width * 0.56
    } else {
        width.min(680.) - 32.
    };
    let (top, bottom) = if landscape {
        (100., height - 30.)
    } else if review {
        (148., 530.)
    } else {
        (220., height - 184.)
    };
    let cell = (region_width / 10.).min((bottom - top) / 11.4);
    let center = if landscape {
        region_width / 2.
    } else {
        width / 2.
    };
    (center - 4. * cell, (top + bottom) / 2. - 4.5 * cell, cell)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn board_fits_portrait_landscape_and_side_panel() {
        for (width, height) in [
            (360., 900.),
            (390., 900.),
            (440., 1000.),
            (825., 600.),
            (1038., 480.),
            (1298., 600.),
            (1900., 600.),
        ] {
            for review in [false, true] {
                let (x, y, cell) = board_geometry(width, height, review);
                let split = height < 800.;
                let right = if split { width * 0.56 } else { width };
                let (top, bottom) = if split {
                    (100., height - 30.)
                } else if review {
                    (148., 530.)
                } else {
                    (220., height - 184.)
                };
                assert!(x - cell >= -0.01);
                assert!(x + cell * 9. <= right + 0.01);
                assert!(y - cell * 1.2 >= top - 0.01);
                assert!(y + cell * 10.2 <= bottom + 0.01);
                assert!(cell > 0.);
            }
        }
    }
}
