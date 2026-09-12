//! zwp_input_method_v2 client: owns the keyboard grab, the virtual keyboard
//! used to forward keys the engine does not want, and the engine itself.

use std::collections::HashSet;
use std::os::fd::{AsFd, OwnedFd};

use popeinput_engine::{InputEngine, Key, Response};
use wayland_client::protocol::wl_keyboard::{KeyState, KeymapFormat};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{delegate_noop, Connection, Dispatch, QueueHandle, WEnum};
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_keyboard_grab_v2::{self, ZwpInputMethodKeyboardGrabV2},
    zwp_input_method_manager_v2::ZwpInputMethodManagerV2,
    zwp_input_method_v2::{self, ZwpInputMethodV2},
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};
use xkbcommon::xkb;

use crate::keys;

pub struct State {
    qh: QueueHandle<Self>,
    input_method: ZwpInputMethodV2,
    /// Held only while a text field is active; the compositor drops the grab
    /// on deactivate, so it is re-requested on every activate.
    grab: Option<ZwpInputMethodKeyboardGrabV2>,
    virtual_keyboard: ZwpVirtualKeyboardV1,
    engine: Box<dyn InputEngine>,

    xkb_context: xkb::Context,
    xkb_state: Option<xkb::State>,
    /// Keymap fd handed to the virtual keyboard; kept alive for its lifetime.
    keymap_fd: Option<OwnedFd>,

    /// Number of `done` events received; the serial expected by `commit`.
    done_serial: u32,
    pending_activate: bool,
    pending_deactivate: bool,
    /// Keycodes whose press we swallowed, so we swallow the matching release.
    consumed_keys: HashSet<u32>,
    /// Keycodes we forwarded as pressed and have not yet released.
    forwarded_keys: HashSet<u32>,
    last_key_time: u32,
    /// Chinese input on; when off every key is forwarded untouched.
    enabled: bool,
    /// A Shift press with no other key since; releasing it toggles `enabled`.
    shift_tap_pending: bool,
    exit: bool,
}

impl State {
    pub fn new(
        qh: &QueueHandle<Self>,
        seat: &wl_seat::WlSeat,
        im_manager: &ZwpInputMethodManagerV2,
        vk_manager: &ZwpVirtualKeyboardManagerV1,
        engine: Box<dyn InputEngine>,
    ) -> Self {
        let input_method = im_manager.get_input_method(seat, qh, ());
        let virtual_keyboard = vk_manager.create_virtual_keyboard(seat, qh, ());
        State {
            qh: qh.clone(),
            input_method,
            grab: None,
            virtual_keyboard,
            engine,
            xkb_context: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            xkb_state: None,
            keymap_fd: None,
            done_serial: 0,
            pending_activate: false,
            pending_deactivate: false,
            consumed_keys: HashSet::new(),
            forwarded_keys: HashSet::new(),
            last_key_time: 0,
            enabled: true,
            shift_tap_pending: false,
            exit: false,
        }
    }

    fn toggle_enabled(&mut self) {
        self.enabled = !self.enabled;
        log::info!("{} input {}", self.engine.name(), if self.enabled { "on" } else { "off" });
        if !self.enabled && self.engine.is_composing() {
            self.engine.reset();
            self.sync_to_client(None);
        }
    }

    pub fn engine_name(&self) -> &str {
        self.engine.name()
    }

    pub fn should_exit(&self) -> bool {
        self.exit
    }

    fn handle_keymap(&mut self, format: WEnum<KeymapFormat>, fd: OwnedFd, size: u32) {
        let WEnum::Value(format) = format else {
            log::warn!("unknown keymap format {format:?}");
            return;
        };
        if format != KeymapFormat::XkbV1 {
            log::warn!("unsupported keymap format {format:?}");
            return;
        }
        let vk_fd = match fd.try_clone() {
            Ok(f) => f,
            Err(e) => {
                log::error!("could not duplicate keymap fd: {e}");
                return;
            }
        };
        // SAFETY: the compositor guarantees `fd` is a readable mapping of `size` bytes.
        let keymap = unsafe {
            xkb::Keymap::new_from_fd(
                &self.xkb_context,
                fd,
                size as usize,
                xkb::KEYMAP_FORMAT_TEXT_V1,
                xkb::KEYMAP_COMPILE_NO_FLAGS,
            )
        };
        match keymap {
            Ok(Some(keymap)) => {
                self.xkb_state = Some(xkb::State::new(&keymap));
                self.virtual_keyboard
                    .keymap(format as u32, vk_fd.as_fd(), size);
                self.keymap_fd = Some(vk_fd);
                log::debug!("keymap loaded ({size} bytes)");
            }
            Ok(None) => log::error!("failed to compile keymap"),
            Err(e) => log::error!("failed to read keymap: {e}"),
        }
    }

    fn forward_key(&mut self, time: u32, key: u32, key_state: KeyState) {
        match key_state {
            KeyState::Pressed => {
                self.forwarded_keys.insert(key);
            }
            _ => {
                self.forwarded_keys.remove(&key);
            }
        }
        self.virtual_keyboard.key(time, key, key_state as u32);
    }

    /// Release every key we forwarded as pressed, so the application never
    /// sees a key stuck down after focus moves away.
    fn release_forwarded_keys(&mut self) {
        let time = self.last_key_time;
        for key in std::mem::take(&mut self.forwarded_keys) {
            self.virtual_keyboard.key(time, key, KeyState::Released as u32);
        }
    }

    fn handle_key(&mut self, time: u32, key: u32, key_state: WEnum<KeyState>) {
        let WEnum::Value(key_state) = key_state else { return };
        self.last_key_time = time;
        let keycode = xkb::Keycode::new(key + 8);

        if key_state == KeyState::Released {
            let was_shift = self
                .xkb_state
                .as_ref()
                .is_some_and(|s| keys::is_shift(keys::keysym(s, keycode)));
            if was_shift && self.shift_tap_pending {
                self.shift_tap_pending = false;
                self.forward_key(time, key, key_state);
                self.toggle_enabled();
                return;
            }
            if self.consumed_keys.remove(&key) {
                return;
            }
            self.forward_key(time, key, key_state);
            return;
        }

        let Some(xkb_state) = self.xkb_state.as_ref() else {
            self.forward_key(time, key, key_state);
            return;
        };
        let input = keys::translate(xkb_state, keycode);
        log::trace!("key {key} -> {input:?}");

        let is_shift = keys::is_shift(keys::keysym(xkb_state, keycode));
        self.shift_tap_pending = is_shift && !input.modifiers.ctrl && !input.modifiers.alt && !input.modifiers.logo;

        if input.key == Key::Space && input.modifiers.ctrl {
            self.consumed_keys.insert(key);
            self.toggle_enabled();
            return;
        }

        if !self.enabled || input.key == Key::Modifier {
            self.forward_key(time, key, key_state);
            return;
        }

        match self.engine.process_key(input) {
            Response::Ignored => self.forward_key(time, key, key_state),
            Response::Consumed | Response::Bell => {
                self.consumed_keys.insert(key);
                self.sync_to_client(None);
            }
            Response::Commit(text) => {
                self.consumed_keys.insert(key);
                self.sync_to_client(Some(&text));
            }
            Response::CommitAndForward(text) => {
                self.sync_to_client(Some(&text));
                self.forward_key(time, key, key_state);
            }
        }
    }

    /// Push the engine's visible state (and an optional commit) to the text field.
    fn sync_to_client(&mut self, commit: Option<&str>) {
        if let Some(text) = commit {
            log::debug!("commit {text:?}");
            self.input_method.commit_string(text.to_string());
        }
        let preedit = self.engine.preedit();
        let candidates = self.engine.candidates();
        let mut shown = preedit.text.clone();
        if !candidates.is_empty() {
            let info = self.engine.page_info();
            let selected = self.engine.selected();
            shown.push(' ');
            for (i, c) in candidates.iter().enumerate() {
                let mark = if i == selected { "▸" } else { "" };
                shown.push_str(&format!("{mark}{}{} ", i + 1, c.text));
            }
            if info.total_pages > 1 {
                shown.push_str(&format!("({}/{})", info.page + 1, info.total_pages));
            }
        }
        let cursor = preedit.cursor as i32;
        self.input_method.set_preedit_string(shown, cursor, cursor);
        self.input_method.commit(self.done_serial);
    }

    fn handle_done(&mut self) {
        self.done_serial = self.done_serial.wrapping_add(1);
        if self.pending_deactivate {
            self.pending_deactivate = false;
            log::debug!("text input deactivated");
            self.drop_grab();
            self.release_forwarded_keys();
            self.engine.reset();
            self.consumed_keys.clear();
        }
        if self.pending_activate {
            self.pending_activate = false;
            log::debug!("text input activated");
            self.drop_grab();
            self.grab = Some(self.input_method.grab_keyboard(&self.qh, ()));
            self.engine.reset();
            self.consumed_keys.clear();
        }
    }

    fn drop_grab(&mut self) {
        if let Some(grab) = self.grab.take() {
            grab.release();
        }
    }
}

impl Dispatch<ZwpInputMethodV2, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZwpInputMethodV2,
        event: zwp_input_method_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwp_input_method_v2::Event;
        match event {
            Event::Activate => state.pending_activate = true,
            Event::Deactivate => state.pending_deactivate = true,
            Event::SurroundingText { .. } | Event::TextChangeCause { .. } | Event::ContentType { .. } => {}
            Event::Done => state.handle_done(),
            Event::Unavailable => {
                log::error!("input method unavailable: another input method is already active on this seat");
                state.exit = true;
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwpInputMethodKeyboardGrabV2, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZwpInputMethodKeyboardGrabV2,
        event: zwp_input_method_keyboard_grab_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwp_input_method_keyboard_grab_v2::Event;
        match event {
            Event::Keymap { format, fd, size } => state.handle_keymap(format, fd, size),
            Event::Key { time, key, state: key_state, .. } => state.handle_key(time, key, key_state),
            Event::Modifiers { mods_depressed, mods_latched, mods_locked, group, .. } => {
                if let Some(xkb_state) = state.xkb_state.as_mut() {
                    xkb_state.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);
                }
                state
                    .virtual_keyboard
                    .modifiers(mods_depressed, mods_latched, mods_locked, group);
            }
            Event::RepeatInfo { .. } => {}
            _ => {}
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, wayland_client::globals::GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &wayland_client::globals::GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(State: ignore ZwpInputMethodManagerV2);
delegate_noop!(State: ignore ZwpVirtualKeyboardManagerV1);
delegate_noop!(State: ignore ZwpVirtualKeyboardV1);

impl Drop for State {
    fn drop(&mut self) {
        self.drop_grab();
        self.release_forwarded_keys();
        self.virtual_keyboard.destroy();
        self.input_method.destroy();
    }
}
