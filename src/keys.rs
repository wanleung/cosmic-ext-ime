use popeinput_engine::{Key, KeyInput, ModifierKey, Modifiers};
use xkbcommon::xkb;

pub fn keysym(state: &xkb::State, keycode: xkb::Keycode) -> xkb::Keysym {
    state.key_get_one_sym(keycode)
}

pub fn is_shift(sym: xkb::Keysym) -> bool {
    matches!(sym, xkb::Keysym::Shift_L | xkb::Keysym::Shift_R)
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
