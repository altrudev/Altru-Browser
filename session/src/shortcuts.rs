//! Keyboard shortcuts. All use the Super (Windows) key, so they never collide with
//! the shortcuts of the tools running inside (DAW transport keys, Krita brushes,
//! editor commands). The one exception is the classic emergency exit.
//!
//! | Keys                  | Does                                   |
//! |-----------------------|----------------------------------------|
//! | Super + Tab           | next window                            |
//! | Super + Shift + Tab   | previous window                        |
//! | Super + 1 … 9         | go to window 1 … 9 (1 is the main one) |
//! | Super + Space         | make this window the main one          |
//! | Super + Enter         | open a terminal                        |
//! | Super + Q             | close this window (the app may ask)    |
//! | Ctrl + Alt + Backspace| leave Altru Session                    |
//! | Ctrl + Alt + F1 … F12 | switch to a text console (on hardware) |

/// Modifier state when a key is pressed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mods {
    pub logo: bool,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    FocusNext,
    FocusPrevious,
    /// Zero-based layout position.
    FocusIndex(usize),
    MakeMain,
    OpenTerminal,
    Close,
    Quit,
    /// Switch to virtual terminal N (hardware backend).
    SwitchVt(i32),
    /// A shortcut key being released: swallowed, does nothing.
    None,
}

// X keysym values (xkbcommon), so this stays testable without a keyboard.
const XK_SPACE: u32 = 0x0020;
const XK_1: u32 = 0x0031;
const XK_9: u32 = 0x0039;
const XK_Q_UPPER: u32 = 0x0051;
const XK_Q: u32 = 0x0071;
const XK_ISO_LEFT_TAB: u32 = 0xfe20;
const XK_BACKSPACE: u32 = 0xff08;
const XK_TAB: u32 = 0xff09;
const XK_RETURN: u32 = 0xff0d;
const XK_KP_ENTER: u32 = 0xff8d;
const XF86_SWITCH_VT_1: u32 = 0x1008_fe01;
const XF86_SWITCH_VT_12: u32 = 0x1008_fe0c;

/// The action for a key press, or `None` to pass the key to the focused app.
/// `keysym` is the symbol with modifiers applied (Shift+Tab is ISO_Left_Tab).
pub fn action_for(mods: Mods, keysym: u32) -> Option<Action> {
    // The keymap turns Ctrl+Alt+F1…F12 into these symbols.
    if (XF86_SWITCH_VT_1..=XF86_SWITCH_VT_12).contains(&keysym) {
        return Some(Action::SwitchVt((keysym - XF86_SWITCH_VT_1 + 1) as i32));
    }
    if mods.ctrl && mods.alt && !mods.logo && keysym == XK_BACKSPACE {
        return Some(Action::Quit);
    }
    if !mods.logo || mods.ctrl || mods.alt {
        return None;
    }
    match keysym {
        XK_TAB if !mods.shift => Some(Action::FocusNext),
        XK_TAB | XK_ISO_LEFT_TAB => Some(Action::FocusPrevious),
        XK_1..=XK_9 if !mods.shift => Some(Action::FocusIndex((keysym - XK_1) as usize)),
        XK_SPACE if !mods.shift => Some(Action::MakeMain),
        XK_RETURN | XK_KP_ENTER if !mods.shift => Some(Action::OpenTerminal),
        XK_Q | XK_Q_UPPER if !mods.shift => Some(Action::Close),
        _ => None,
    }
}

/// Index after moving `step` places from `current` among `count` windows, wrapping.
pub fn cycle(current: Option<usize>, count: usize, forward: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(match (current, forward) {
        (None, true) => 0,
        (None, false) => count - 1,
        (Some(i), true) => (i + 1) % count,
        (Some(i), false) => (i + count - 1) % count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUPER: Mods = Mods {
        logo: true,
        shift: false,
        ctrl: false,
        alt: false,
    };

    #[test]
    fn super_shortcuts() {
        assert_eq!(action_for(SUPER, XK_TAB), Some(Action::FocusNext));
        let super_shift = Mods {
            shift: true,
            ..SUPER
        };
        assert_eq!(
            action_for(super_shift, XK_ISO_LEFT_TAB),
            Some(Action::FocusPrevious)
        );
        assert_eq!(action_for(SUPER, XK_1), Some(Action::FocusIndex(0)));
        assert_eq!(action_for(SUPER, XK_9), Some(Action::FocusIndex(8)));
        assert_eq!(action_for(SUPER, XK_SPACE), Some(Action::MakeMain));
        assert_eq!(action_for(SUPER, XK_RETURN), Some(Action::OpenTerminal));
        assert_eq!(action_for(SUPER, XK_Q), Some(Action::Close));
    }

    #[test]
    fn app_keys_pass_through() {
        // Plain keys and app shortcuts (Ctrl+Q, Ctrl+Space, Tab) reach the app.
        assert_eq!(action_for(Mods::default(), XK_Q), None);
        assert_eq!(
            action_for(
                Mods {
                    ctrl: true,
                    ..Mods::default()
                },
                XK_Q
            ),
            None
        );
        assert_eq!(
            action_for(
                Mods {
                    ctrl: true,
                    ..Mods::default()
                },
                XK_SPACE
            ),
            None
        );
        assert_eq!(action_for(Mods::default(), XK_TAB), None);
        assert_eq!(
            action_for(
                Mods {
                    ctrl: true,
                    ..SUPER
                },
                XK_Q
            ),
            None
        );
        assert_eq!(
            action_for(
                Mods {
                    shift: true,
                    ..SUPER
                },
                XK_Q_UPPER
            ),
            None
        );
    }

    #[test]
    fn emergency_exit() {
        let ctrl_alt = Mods {
            ctrl: true,
            alt: true,
            ..Mods::default()
        };
        assert_eq!(action_for(ctrl_alt, XK_BACKSPACE), Some(Action::Quit));
        assert_eq!(
            action_for(
                Mods {
                    ctrl: true,
                    ..Mods::default()
                },
                XK_BACKSPACE
            ),
            None
        );
    }

    #[test]
    fn console_switching() {
        let ctrl_alt = Mods {
            ctrl: true,
            alt: true,
            ..Mods::default()
        };
        assert_eq!(
            action_for(ctrl_alt, XF86_SWITCH_VT_1),
            Some(Action::SwitchVt(1))
        );
        assert_eq!(
            action_for(ctrl_alt, XF86_SWITCH_VT_12),
            Some(Action::SwitchVt(12))
        );
    }

    #[test]
    fn cycling_wraps() {
        assert_eq!(cycle(None, 0, true), None);
        assert_eq!(cycle(None, 3, true), Some(0));
        assert_eq!(cycle(None, 3, false), Some(2));
        assert_eq!(cycle(Some(2), 3, true), Some(0));
        assert_eq!(cycle(Some(0), 3, false), Some(2));
    }
}
