//! The menu-bar icon: a small waveform glyph drawn in code, so the repository holds no binary
//! image (docs/UX.md, "Menu-bar icon"). It is a template image: macOS tints it for light and
//! dark menu bars.

/// Side of the square icon in pixels (18 pt at 2x).
pub const SIZE: u32 = 36;

/// Bar heights of the waveform, as a fraction of the icon's height.
const BARS: [f64; 5] = [0.35, 0.65, 1.0, 0.65, 0.35];

/// RGBA pixels of the waveform: black bars on transparent.
pub fn waveform() -> Vec<u8> {
    let size = SIZE as usize;
    let mut rgba = vec![0u8; size * size * 4];
    let bar_width = 4;
    let gap = 3;
    let total = BARS.len() * bar_width + (BARS.len() - 1) * gap;
    let left = (size - total) / 2;
    let usable = size as f64 * 0.8;
    for (i, height) in BARS.iter().enumerate() {
        let h = (usable * height).round() as usize;
        let top = (size - h) / 2;
        let x0 = left + i * (bar_width + gap);
        for y in top..top + h {
            for x in x0..x0 + bar_width {
                let p = (y * size + x) * 4;
                rgba[p + 3] = 255;
            }
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_waveform_has_five_symmetric_bars() {
        let rgba = waveform();
        assert_eq!(rgba.len(), (SIZE * SIZE * 4) as usize);
        let opaque = |x: usize, y: usize| rgba[(y * SIZE as usize + x) * 4 + 3] == 255;
        let middle = SIZE as usize / 2;
        let columns: Vec<bool> = (0..SIZE as usize).map(|x| opaque(x, middle)).collect();
        let bars = columns.windows(2).filter(|w| !w[0] && w[1]).count();
        assert_eq!(bars, 5);
        let mirrored: Vec<bool> = columns.iter().rev().copied().collect();
        assert_eq!(columns, mirrored);
        // The centre bar is the tallest.
        assert!(opaque(middle, 4) && !opaque(1, 4));
    }
}
