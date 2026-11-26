//! Where the pill goes: bottom centre of the screen's usable area (docs/UX.md, "The pill").

/// A rectangle in logical points, origin top-left, as Tauri reports monitors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The pill's size in points (docs/UX.md: about 120 by 32 pt).
pub const PILL_SIZE: (f64, f64) = (120.0, 32.0);
/// Gap between the pill and the bottom of the usable area (above the Dock, if shown).
pub const BOTTOM_MARGIN: f64 = 12.0;

/// The top-left corner of a `size` window centred at the bottom of `work_area`.
pub fn bottom_centre(work_area: Rect, size: (f64, f64)) -> (f64, f64) {
    let x = work_area.x + (work_area.width - size.0) / 2.0;
    let y = work_area.y + work_area.height - size.1 - BOTTOM_MARGIN;
    (x.round(), y.round())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// docs/UX.md: a small capsule at the bottom centre.
    #[test]
    fn the_pill_sits_at_the_bottom_centre() {
        let screen = Rect {
            x: 0.0,
            y: 25.0,
            width: 1512.0,
            height: 913.0,
        };
        assert_eq!(bottom_centre(screen, PILL_SIZE), (696.0, 894.0));
    }

    #[test]
    fn a_second_screen_to_the_right_keeps_its_offset() {
        let screen = Rect {
            x: 1512.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let (x, y) = bottom_centre(screen, PILL_SIZE);
        assert_eq!((x, y), (1512.0 + 900.0, 1080.0 - 32.0 - BOTTOM_MARGIN));
    }
}
