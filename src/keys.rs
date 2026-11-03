use cosmic_ext_ime_engine::{Key, KeyInput, ModifierKey, Modifiers};
use xkbcommon::xkb;

pub fn keysym(state: &xkb::State, keycode: xkb::Keycode) -> xkb::Keysym {
    state.key_get_one_sym(keycode)
}

pub fn is_shift(sym: xkb::Keysym) -> bool {
    matches!(sym, xkb::Keysym::Shift_L | xkb::Keysym::Shift_R)
}

/// Bit of the Lock modifier in this keymap's serialized masks.
pub fn caps_mask(keymap: &xkb::Keymap) -> u32 {
    let idx = keymap.mod_get_index(xkb::MOD_NAME_CAPS);
    if idx == xkb::MOD_INVALID {
        0
    } else {
        1 << idx
    }
}

pub fn translate(state: &xkb::State, keycode: xkb::Keycode) -> KeyInput {
    let sym = keysym(state, keycode);
    let modifiers = Modifiers {
        shift: state.mod_name_is_active(xkb::MOD_NAME_SHIFT, xkb::STATE_MODS_EFFECTIVE),
        ctrl: state.mod_name_is_active(xkb::MOD_NAME_CTRL, xkb::STATE_MODS_EFFECTIVE),
        alt: state.mod_name_is_active(xkb::MOD_NAME_ALT, xkb::STATE_MODS_EFFECTIVE),
        logo: state.mod_name_is_active(xkb::MOD_NAME_LOGO, xkb::STATE_MODS_EFFECTIVE),
    };

    let key = match sym {
        xkb::Keysym::BackSpace => Key::Backspace,
        xkb::Keysym::Escape => Key::Escape,
        xkb::Keysym::space => Key::Space,
        xkb::Keysym::Return | xkb::Keysym::KP_Enter => Key::Enter,
        xkb::Keysym::Tab => Key::Tab,
        xkb::Keysym::Up => Key::Up,
        xkb::Keysym::Down => Key::Down,
        xkb::Keysym::Left => Key::Left,
        xkb::Keysym::Right => Key::Right,
        xkb::Keysym::Page_Up => Key::PageUp,
        xkb::Keysym::Page_Down => Key::PageDown,
        xkb::Keysym::Delete => Key::Delete,
        xkb::Keysym::Home => Key::Home,
        xkb::Keysym::End => Key::End,
        xkb::Keysym::Caps_Lock => Key::CapsLock,
        // Keypad: digits and operators must not become bopomofo keys, and
        // ibus-chewing routes the navigation cluster like the normal keys.
        xkb::Keysym::KP_Space => Key::Space,
        xkb::Keysym::KP_Tab => Key::Tab,
        xkb::Keysym::KP_Home => Key::Home,
        xkb::Keysym::KP_End => Key::End,
        xkb::Keysym::KP_Up => Key::Up,
        xkb::Keysym::KP_Down => Key::Down,
        xkb::Keysym::KP_Left => Key::Left,
        xkb::Keysym::KP_Right => Key::Right,
        xkb::Keysym::KP_Page_Up => Key::PageUp,
        xkb::Keysym::KP_Page_Down => Key::PageDown,
        xkb::Keysym::KP_Delete => Key::Delete,
        xkb::Keysym::KP_Multiply => Key::KeypadSymbol('*'),
        xkb::Keysym::KP_Add => Key::KeypadSymbol('+'),
        xkb::Keysym::KP_Separator => Key::KeypadSymbol(','),
        xkb::Keysym::KP_Subtract => Key::KeypadSymbol('-'),
        xkb::Keysym::KP_Decimal => Key::KeypadSymbol('.'),
        xkb::Keysym::KP_Divide => Key::KeypadSymbol('/'),
        s if (xkb::Keysym::KP_0..=xkb::Keysym::KP_9).contains(&s) => {
            Key::KeypadDigit((s.raw() - xkb::Keysym::KP_0.raw()) as u8)
        }
        xkb::Keysym::Shift_L => Key::Modifier(ModifierKey::ShiftL),
        xkb::Keysym::Shift_R => Key::Modifier(ModifierKey::ShiftR),
        xkb::Keysym::Control_L => Key::Modifier(ModifierKey::ControlL),
        xkb::Keysym::Control_R => Key::Modifier(ModifierKey::ControlR),
        xkb::Keysym::Alt_L => Key::Modifier(ModifierKey::AltL),
        xkb::Keysym::Alt_R => Key::Modifier(ModifierKey::AltR),
        xkb::Keysym::Super_L => Key::Modifier(ModifierKey::SuperL),
        xkb::Keysym::Super_R => Key::Modifier(ModifierKey::SuperR),
        s if s.is_modifier_key() => Key::Modifier(ModifierKey::Other),
        _ => {
            let utf8 = state.key_get_utf8(keycode);
            let mut chars = utf8.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if !c.is_control() => Key::Char(c),
                _ => Key::Other,
            }
        }
    };

    KeyInput { key, modifiers }
}

#[cfg(test)]
mod tests {
    //! Headless tests for the keysym -> Key translation, over a real
    //! xkbcommon keymap.  Keycodes are evdev codes + 8, like the kernel.

    use super::*;
    use xkbcommon::xkb::{KeyDirection, Keycode};

    const KEY_A: u16 = 30;
    const KEY_LEFTSHIFT: u16 = 42;
    const KEY_CAPSLOCK: u16 = 58;
    const KEY_SPACE: u16 = 57;
    const KEY_BACKSPACE: u16 = 14;
    const KEY_ESCAPE: u16 = 1;
    const KEY_TAB: u16 = 15;
    const KEY_ENTER: u16 = 28;
    const KEY_COMMA: u16 = 51;
    const KEY_HOME: u16 = 102;
    const KEY_END: u16 = 107;
    const KEY_PAGEUP: u16 = 104;
    const KEY_PAGEDOWN: u16 = 109;
    const KEY_DELETE: u16 = 111;
    const KEY_LEFT: u16 = 105;
    const KEY_KP1: u16 = 79;
    const KP_ASTERISK: u16 = 55;
    const KP_PLUS: u16 = 78;
    const KP_MINUS: u16 = 74;
    const KP_DOT: u16 = 83;
    const KP_SLASH: u16 = 98;
    const KEY_NUMLOCK: u16 = 69;
    const KEY_LEFTCTRL: u16 = 29;
    const KEY_LEFTMETA: u16 = 125;

    fn state() -> xkb::State {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(
            &context,
            "evdev",
            "",
            "us",
            "",
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .expect("us keymap should compile");
        xkb::State::new(&keymap)
    }

    fn press(st: &mut xkb::State, evdev: u16) {
        st.update_key(Keycode::new((evdev + 8) as u32), KeyDirection::Down);
    }

    fn release(st: &mut xkb::State, evdev: u16) {
        st.update_key(Keycode::new((evdev + 8) as u32), KeyDirection::Up);
    }

    #[test]
    fn letter_translates_to_char() {
        let st = state();
        let input = translate(&st, Keycode::new((KEY_A + 8) as u32));
        assert_eq!(input.key, Key::Char('a'));
        assert!(!input.modifiers.shift);
    }

    #[test]
    fn shifted_letter_reports_shift_and_uppercase() {
        let mut st = state();
        press(&mut st, KEY_LEFTSHIFT);
        let input = translate(&st, Keycode::new((KEY_A + 8) as u32));
        assert_eq!(input.key, Key::Char('A'));
        assert!(input.modifiers.shift);
    }

    #[test]
    fn shifted_comma_is_less_than() {
        let mut st = state();
        press(&mut st, KEY_LEFTSHIFT);
        let input = translate(&st, Keycode::new((KEY_COMMA + 8) as u32));
        assert_eq!(input.key, Key::Char('<'));
        assert!(input.modifiers.shift);
    }

    #[test]
    fn special_keys_map() {
        let st = state();
        for (evdev, want) in [
            (KEY_BACKSPACE, Key::Backspace),
            (KEY_ESCAPE, Key::Escape),
            (KEY_TAB, Key::Tab),
            (KEY_ENTER, Key::Enter),
            (KEY_SPACE, Key::Space),
            (KEY_HOME, Key::Home),
            (KEY_END, Key::End),
            (KEY_PAGEUP, Key::PageUp),
            (KEY_PAGEDOWN, Key::PageDown),
            (KEY_DELETE, Key::Delete),
            (KEY_LEFT, Key::Left),
            (KEY_CAPSLOCK, Key::CapsLock),
        ] {
            let input = translate(&st, Keycode::new((evdev + 8) as u32));
            assert_eq!(input.key, want, "evdev {evdev}");
        }
    }

    #[test]
    fn modifier_keys_map_to_modifier() {
        let st = state();
        let input = translate(&st, Keycode::new((KEY_LEFTSHIFT + 8) as u32));
        assert_eq!(input.key, Key::Modifier(ModifierKey::ShiftL));
        let input = translate(&st, Keycode::new((KEY_LEFTMETA + 8) as u32));
        assert!(matches!(input.key, Key::Modifier(_)));
    }

    /// Caps Lock is a lock: one press+release locks it, the next unlocks.
    /// Lock stays depressed while the key is held, which is why the
    /// frontend reads the lock only once the key is up.
    #[test]
    fn caps_lock_toggles_locked_state() {
        let mut st = state();
        let lock = caps_mask(&st.get_keymap());
        assert_eq!(lock, 0x2, "Lock is bit 1 in xkb keymaps");
        let locked = |st: &xkb::State| st.serialize_mods(xkb::STATE_MODS_LOCKED) & lock != 0;
        let held = |st: &xkb::State| st.serialize_mods(xkb::STATE_MODS_DEPRESSED) & lock != 0;
        assert!(!locked(&st));
        press(&mut st, KEY_CAPSLOCK);
        assert!(held(&st), "Lock sits in depressed while the key is down");
        release(&mut st, KEY_CAPSLOCK);
        assert!(!held(&st));
        assert!(locked(&st));
        // With Caps Lock on, letter keysyms arrive uppercase WITHOUT Shift:
        // the engine normalises case by Shift alone in Chinese mode.
        let input = translate(&st, Keycode::new((KEY_A + 8) as u32));
        assert_eq!(input.key, Key::Char('A'));
        assert!(!input.modifiers.shift);
        // Unlocking: still locked while the key is held, cleared on release.
        press(&mut st, KEY_CAPSLOCK);
        assert!(held(&st));
        assert!(locked(&st), "unlock happens on release (journal-verified)");
        release(&mut st, KEY_CAPSLOCK);
        assert!(!locked(&st));
    }

    #[test]
    fn ctrl_reports_control_modifier() {
        let mut st = state();
        press(&mut st, KEY_LEFTCTRL);
        let input = translate(&st, Keycode::new((KEY_A + 8) as u32));
        assert!(input.modifiers.ctrl);
        assert!(!input.modifiers.shift);
    }

    /// Keypad digits must NEVER arrive as bopomofo-producing Char keys;
    /// they get their own variants so the engine can treat them like
    /// ibus-chewing's keypad rules.
    #[test]
    fn keypad_digits_with_numlock() {
        let mut st = state();
        press(&mut st, KEY_NUMLOCK); // digits on the keypad
        let input = translate(&st, Keycode::new((KEY_KP1 + 8) as u32));
        assert_eq!(input.key, Key::KeypadDigit(1), "KP_1 with NumLock");
        release(&mut st, KEY_NUMLOCK);
        let input = translate(&st, Keycode::new((KEY_KP1 + 8) as u32));
        // Whether or not NumLock is still locked, KP_1 is never a Char.
        assert!(
            matches!(input.key, Key::KeypadDigit(1) | Key::End),
            "KP_1 translated to {:?}",
            input.key
        );
    }

    #[test]
    fn keypad_operators_map_to_symbols() {
        let mut st = state();
        press(&mut st, KEY_NUMLOCK);
        for (evdev, c) in [
            (KP_ASTERISK, '*'),
            (KP_PLUS, '+'),
            (KP_MINUS, '-'),
            (KP_DOT, '.'),
            (KP_SLASH, '/'),
        ] {
            let input = translate(&st, Keycode::new((evdev + 8) as u32));
            assert_eq!(input.key, Key::KeypadSymbol(c), "evdev {evdev}");
        }
    }
}
