//! Automatic window layout: no dragging, no overlapping, no lost windows.
//!
//! One window fills the screen. With more, the first (the main tool: the DAW, the
//! canvas, the editor) takes the left side and the rest stack on the right, so
//! helpers (a mixer, a reference image, a terminal) stay visible next to it.
//! Pure geometry, no compositor types, so it can be tested on its own.

/// A rectangle in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Share of the screen width given to the main window when others are open.
pub const MAIN_SHARE: f32 = 0.62;
/// Space between windows and around the screen edge.
pub const GAP: i32 = 6;

/// Tiles for `count` windows on a `width` x `height` screen, in window order.
pub fn tiles(count: usize, width: i32, height: i32) -> Vec<Tile> {
    if count == 0 || width <= 2 * GAP || height <= 2 * GAP {
        return Vec::new();
    }
    let inner_w = width - 2 * GAP;
    let inner_h = height - 2 * GAP;
    if count == 1 {
        return vec![Tile {
            x: GAP,
            y: GAP,
            width: inner_w,
            height: inner_h,
        }];
    }
    let main_w = ((inner_w - GAP) as f32 * MAIN_SHARE).round() as i32;
    let side_x = GAP + main_w + GAP;
    let side_w = inner_w - main_w - GAP;
    let stack = (count - 1) as i32;
    let slot_h = (inner_h - GAP * (stack - 1)) / stack;
    let mut out = vec![Tile {
        x: GAP,
        y: GAP,
        width: main_w,
        height: inner_h,
    }];
    for index in 0..stack {
        let y = GAP + index * (slot_h + GAP);
        // The last slot absorbs rounding so the stack always ends at the bottom gap.
        let h = if index == stack - 1 {
            height - GAP - y
        } else {
            slot_h
        };
        out.push(Tile {
            x: side_x,
            y,
            width: side_w,
            height: h.max(1),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlaps(a: &Tile, b: &Tile) -> bool {
        a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
    }

    #[test]
    fn one_window_fills_the_screen() {
        assert_eq!(
            tiles(1, 1920, 1080),
            [Tile {
                x: GAP,
                y: GAP,
                width: 1920 - 2 * GAP,
                height: 1080 - 2 * GAP
            }]
        );
    }

    #[test]
    fn main_window_left_helpers_stacked_right() {
        let t = tiles(4, 1920, 1080);
        assert_eq!(t.len(), 4);
        assert!(t[0].width > t[1].width, "main is wider");
        for helper in &t[1..] {
            assert_eq!(helper.x, t[0].x + t[0].width + GAP);
            assert_eq!(helper.x + helper.width, 1920 - GAP);
        }
        assert_eq!(t[3].y + t[3].height, 1080 - GAP, "stack reaches the bottom");
    }

    #[test]
    fn never_overlaps_and_stays_on_screen() {
        for count in 1..12 {
            for (w, h) in [(1366, 768), (1600, 900), (3840, 2160), (800, 600)] {
                let t = tiles(count, w, h);
                assert_eq!(t.len(), count);
                for (i, a) in t.iter().enumerate() {
                    assert!(a.x >= 0 && a.y >= 0 && a.x + a.width <= w && a.y + a.height <= h);
                    assert!(a.width > 0 && a.height > 0);
                    for b in &t[i + 1..] {
                        assert!(!overlaps(a, b), "{count} windows at {w}x{h}: {a:?} {b:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn degenerate_screens_give_nothing() {
        assert!(tiles(0, 1920, 1080).is_empty());
        assert!(tiles(3, 10, 10).is_empty());
    }
}
