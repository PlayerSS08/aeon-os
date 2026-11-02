//! Choosing a screen mode: the pure part. The firmware lists its modes; Aeon wants the largest one
//! that has a linear framebuffer, so the console fills the whole panel. Host-testable.

/// One mode the firmware offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeInfo {
    pub index: u32,
    pub width: u32,
    pub height: u32,
    /// False for blt-only modes (no framebuffer we can write to).
    pub linear: bool,
}

/// Picks a mode number.
///
/// 1. A size asked for with `mode=WxH` wins when a linear mode of that size exists.
/// 2. Otherwise the largest linear mode wins; on a tie the current mode is kept (no needless switch).
///
/// Returns `None` when no linear mode exists at all (the caller then uses the firmware console).
pub fn choose(modes: &[ModeInfo], current: u32, want: Option<(u32, u32)>) -> Option<u32> {
    if let Some((w, h)) = want {
        if let Some(m) = modes.iter().find(|m| m.linear && m.width == w && m.height == h) {
            return Some(m.index);
        }
    }
    let mut best: Option<&ModeInfo> = None;
    for m in modes.iter().filter(|m| m.linear && m.width >= 320 && m.height >= 200) {
        let area = u64::from(m.width) * u64::from(m.height);
        let better = match best {
            None => true,
            Some(b) => {
                let ba = u64::from(b.width) * u64::from(b.height);
                area > ba || (area == ba && m.index == current && b.index != current)
            }
        };
        if better {
            best = Some(m);
        }
    }
    best.map(|m| m.index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(index: u32, width: u32, height: u32) -> ModeInfo {
        ModeInfo { index, width, height, linear: true }
    }

    #[test]
    fn largest_linear_mode_wins_even_when_the_current_one_is_usable() {
        let modes = [m(0, 800, 600), m(1, 1024, 768), m(2, 1920, 1080)];
        assert_eq!(choose(&modes, 1, None), Some(2));
    }

    #[test]
    fn a_small_current_mode_is_replaced() {
        let modes = [m(0, 640, 480), m(1, 1366, 768)];
        assert_eq!(choose(&modes, 0, None), Some(1));
    }

    #[test]
    fn the_wanted_size_beats_a_larger_mode() {
        let modes = [m(0, 800, 600), m(1, 1920, 1080)];
        assert_eq!(choose(&modes, 1, Some((800, 600))), Some(0));
    }

    #[test]
    fn a_wanted_size_the_firmware_lacks_falls_back_to_the_largest() {
        let modes = [m(0, 800, 600), m(1, 1920, 1080)];
        assert_eq!(choose(&modes, 0, Some((1280, 720))), Some(1));
    }

    #[test]
    fn a_tie_keeps_the_current_mode() {
        let modes = [m(3, 1920, 1080), m(7, 1920, 1080)];
        assert_eq!(choose(&modes, 7, None), Some(7));
        assert_eq!(choose(&modes, 3, None), Some(3));
    }

    #[test]
    fn blt_only_modes_are_never_chosen() {
        let modes = [ModeInfo { index: 0, width: 3840, height: 2160, linear: false }, m(1, 1024, 768)];
        assert_eq!(choose(&modes, 0, None), Some(1));
        assert_eq!(choose(&modes, 0, Some((3840, 2160))), Some(1));
    }

    #[test]
    fn no_modes_or_no_linear_mode_gives_none() {
        assert_eq!(choose(&[], 0, None), None);
        let only_blt = [ModeInfo { index: 0, width: 1024, height: 768, linear: false }];
        assert_eq!(choose(&only_blt, 0, None), None);
    }

    #[test]
    fn tiny_modes_are_ignored() {
        let modes = [m(0, 200, 100)];
        assert_eq!(choose(&modes, 0, None), None);
    }
}
