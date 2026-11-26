//! The menu-bar icon: the speed pill (docs/brand/logo, direction E4) drawn in code, so the
//! repository holds no binary image for it (docs/UX.md, "Menu-bar icon"). It is a template
//! image: macOS tints it for light and dark menu bars.

/// Side of the square icon in pixels (18 pt at 2x).
pub const SIZE: u32 = 36;

/// The part of the 1024 logo grid the icon shows, as in `docs/brand/logo/e4-speed-pill-menubar.svg`.
const VIEW_ORIGIN: f64 = 232.0;
const VIEW_SIDE: f64 = 560.0;

/// Horizontal capsules on the logo grid: left, top, right, bottom and opacity. The two short
/// trails come first, then the pill.
const CAPSULES: [(f64, f64, f64, f64, f64); 3] = [
    (250.0, 440.0, 370.0, 496.0, 0.55),
    (300.0, 528.0, 390.0, 584.0, 0.35),
    (420.0, 404.0, 774.0, 620.0, 1.0),
];

/// The dot, cut out of the pill: centre x, centre y and radius on the logo grid.
const DOT: (f64, f64, f64) = (666.0, 512.0, 48.0);

/// Signed distance from a point to a horizontal capsule: negative inside.
fn capsule_distance(
    x: f64,
    y: f64,
    (left, top, right, bottom, _): (f64, f64, f64, f64, f64),
) -> f64 {
    let radius = (bottom - top) / 2.0;
    let cy = top + radius;
    let cx = x.clamp(left + radius, right - radius);
    ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - radius
}

/// RGBA pixels of the speed pill: black on transparent, edges antialiased.
pub fn speed_pill() -> Vec<u8> {
    let size = SIZE as usize;
    let unit = VIEW_SIDE / SIZE as f64;
    let coverage = |distance: f64| (0.5 - distance / unit).clamp(0.0, 1.0);
    let mut rgba = vec![0u8; size * size * 4];
    for py in 0..size {
        for px in 0..size {
            let x = VIEW_ORIGIN + (px as f64 + 0.5) * unit;
            let y = VIEW_ORIGIN + (py as f64 + 0.5) * unit;
            let hole = coverage(((x - DOT.0).powi(2) + (y - DOT.1).powi(2)).sqrt() - DOT.2);
            let alpha = CAPSULES
                .iter()
                .enumerate()
                .map(|(i, &shape)| {
                    let a = coverage(capsule_distance(x, y, shape)) * shape.4;
                    if i == CAPSULES.len() - 1 {
                        a * (1.0 - hole)
                    } else {
                        a
                    }
                })
                .fold(0.0, f64::max);
            rgba[(py * size + px) * 4 + 3] = (alpha * 255.0).round() as u8;
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pixel position of a point on the logo grid.
    fn pixel(x: f64, y: f64) -> (usize, usize) {
        let unit = VIEW_SIDE / SIZE as f64;
        (
            ((x - VIEW_ORIGIN) / unit) as usize,
            ((y - VIEW_ORIGIN) / unit) as usize,
        )
    }

    #[test]
    fn the_speed_pill_has_a_solid_pill_a_cut_out_dot_and_fainter_trails() {
        let rgba = speed_pill();
        assert_eq!(rgba.len(), (SIZE * SIZE * 4) as usize);
        let alpha = |(x, y): (usize, usize)| rgba[(y * SIZE as usize + x) * 4 + 3];
        // The pill's body, left of the dot, is solid.
        assert_eq!(alpha(pixel(500.0, 512.0)), 255);
        // The dot is cut out of it.
        assert_eq!(alpha(pixel(666.0, 512.0)), 0);
        // The trails show, fainter than the pill, the lower one faintest.
        let upper = alpha(pixel(310.0, 468.0));
        let lower = alpha(pixel(345.0, 556.0));
        assert!(upper > lower && lower > 0 && upper < 255, "{upper} {lower}");
        // Corners stay empty.
        assert_eq!(alpha((0, 0)), 0);
        assert_eq!(alpha((SIZE as usize - 1, SIZE as usize - 1)), 0);
    }
}
