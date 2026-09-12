//! zwp_input_method_v2 client: owns the keyboard grab, the virtual keyboard
//! used to forward keys the engine does not want, the candidate popup, and
//! the engine itself.

use std::collections::HashSet;
use std::os::fd::{AsFd, OwnedFd};
use std::time::Duration;

use calloop::timer::{TimeoutAction, Timer};
use calloop::{LoopHandle, RegistrationToken};

use popeinput_engine::{InputEngine, Key, KeyInput, Response};
use wayland_client::protocol::wl_keyboard::{KeyState, KeymapFormat};
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
};
use wayland_client::{delegate_noop, Connection, Dispatch, QueueHandle, WEnum};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
    wp_fractional_scale_v1::{self, WpFractionalScaleV1},
};
use wayland_protocols::wp::viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter};
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_keyboard_grab_v2::{self, ZwpInputMethodKeyboardGrabV2},
    zwp_input_method_manager_v2::ZwpInputMethodManagerV2,
    zwp_input_method_v2::{self, ZwpInputMethodV2},
    zwp_input_popup_surface_v2::{self, ZwpInputPopupSurfaceV2},
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};
use xkbcommon::xkb;

use popeinput_config::PopeinputConfig;

use crate::config::build_engine;
use crate::keys;
use crate::popup::{Popup, Style};

pub struct State {
    qh: QueueHandle<Self>,
    cfg: PopeinputConfig,
    input_method: ZwpInputMethodV2,
    /// Held only while a text field is active; the compositor drops the grab
    /// on deactivate, so it is re-requested on every activate.
    grab: Option<ZwpInputMethodKeyboardGrabV2>,
    virtual_keyboard: ZwpVirtualKeyboardV1,
    popup: Popup<Self>,
    engine: Box<dyn InputEngine>,

    xkb_context: xkb::Context,
    keymap: Option<xkb::Keymap>,
    xkb_state: Option<xkb::State>,
    /// Keymap fd handed to the virtual keyboard; kept alive for its lifetime.
    keymap_fd: Option<OwnedFd>,
    /// Chinese input follows the active xkb layout (needs >1 layout configured).
    follow_layout: bool,

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
    loop_handle: Option<LoopHandle<'static, State>>,
    /// Wayland leaves key repeat to clients, so consumed keys are repeated
    /// here from the compositor's repeat_info.
    repeat: Option<(u32, KeyInput)>,
    repeat_timer: Option<RegistrationToken>,
    repeat_rate: Option<(Duration, Duration)>,
    exit: bool,
}

impl State {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        qh: &QueueHandle<Self>,
        seat: &wl_seat::WlSeat,
        compositor: &wl_compositor::WlCompositor,
        shm: &wl_shm::WlShm,
        fractional_scale: Option<&WpFractionalScaleManagerV1>,
        viewporter: Option<&WpViewporter>,
        im_manager: &ZwpInputMethodManagerV2,
        vk_manager: &ZwpVirtualKeyboardManagerV1,
        engine: Box<dyn InputEngine>,
        cfg: PopeinputConfig,
    ) -> Self {
        let input_method = im_manager.get_input_method(seat, qh, ());
        let virtual_keyboard = vk_manager.create_virtual_keyboard(seat, qh, ());
        let popup = Popup::new(
            qh,
            compositor,
            shm,
            fractional_scale,
            viewporter,
            &input_method,
            Style::from_cosmic(cfg.popup_font_size as f32),
        );
        State {
            qh: qh.clone(),
            cfg,
            input_method,
            grab: None,
            virtual_keyboard,
            popup,
            engine,
            xkb_context: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            keymap: None,
            xkb_state: None,
            keymap_fd: None,
            follow_layout: false,
            done_serial: 0,
            pending_activate: false,
            pending_deactivate: false,
            consumed_keys: HashSet::new(),
            forwarded_keys: HashSet::new(),
            last_key_time: 0,
            enabled: true,
            shift_tap_pending: false,
            loop_handle: None,
            repeat: None,
            repeat_timer: None,
            repeat_rate: None,
            exit: false,
        }
    }

    pub fn set_loop_handle(&mut self, handle: LoopHandle<'static, State>) {
        self.loop_handle = Some(handle);
    }

    fn start_repeat(&mut self, key: u32, input: KeyInput) {
        self.stop_repeat();
        let (Some(handle), Some((delay, interval))) = (self.loop_handle.clone(), self.repeat_rate) else {
            return;
        };
        self.repeat = Some((key, input));
        let token = handle.insert_source(Timer::from_duration(delay), move |_, _, state: &mut State| {
            match state.repeat {
                Some((k, input)) if k == key => {
                    let response = state.engine.process_key(input);
                    let commit = match &response {
                        Response::Commit(t) | Response::CommitAndForward(t) => Some(t.clone()),
                        _ => None,
                    };
                    if response != Response::Ignored {
                        state.sync_to_client(commit.as_deref());
                    }
                    TimeoutAction::ToDuration(interval)
                }
                _ => TimeoutAction::Drop,
            }
        });
        match token {
            Ok(t) => self.repeat_timer = Some(t),
            Err(e) => log::warn!("key repeat timer: {e}"),
        }
    }

    fn stop_repeat(&mut self) {
        self.repeat = None;
        if let (Some(handle), Some(token)) = (self.loop_handle.as_ref(), self.repeat_timer.take()) {
            handle.remove(token);
        }
    }

    pub fn engine_name(&self) -> &str {
        self.engine.name()
    }

    pub fn should_exit(&self) -> bool {
        self.exit
    }

    /// Apply settings changed on disk without restarting.
    pub fn apply_config(&mut self, cfg: PopeinputConfig) {
        if cfg == self.cfg {
            return;
        }
        if cfg.engine_fields() != self.cfg.engine_fields() {
            match build_engine(&cfg) {
                Ok(engine) => {
                    self.engine = engine;
                    self.sync_to_client(None);
                    log::info!("engine reloaded ({})", self.engine.name());
                }
                Err(e) => log::error!("keeping previous engine: {e:#}"),
            }
        }
        if cfg.popup_font_size != self.cfg.popup_font_size {
            self.popup.set_style(Style::from_cosmic(cfg.popup_font_size as f32));
        }
        self.cfg = cfg;
        self.follow_layout = self.cfg.follow_layout
            && self.keymap.as_ref().is_some_and(|k| k.num_layouts() > 1);
        if self.follow_layout {
            self.apply_layout();
        } else {
            self.set_enabled(true);
        }
    }

    /// The COSMIC theme changed on disk.
    pub fn reload_theme(&mut self) {
        self.popup.set_style(Style::from_cosmic(self.cfg.popup_font_size as f32));
    }

    fn set_enabled(&mut self, enabled: bool) {
        if self.enabled == enabled {
            return;
        }
        self.enabled = enabled;
        log::info!("{} input {}", self.engine.name(), if enabled { "on" } else { "off" });
        if !enabled && self.engine.is_composing() {
            self.engine.reset();
            self.sync_to_client(None);
        }
    }

    fn toggle_enabled(&mut self) {
        self.set_enabled(!self.enabled);
    }

    /// Re-evaluate `enabled` from the active layout name when following layouts.
    fn apply_layout(&mut self) {
        if !self.follow_layout {
            return;
        }
        let (Some(keymap), Some(state)) = (&self.keymap, &self.xkb_state) else { return };
        let group = state.serialize_layout(xkb::STATE_LAYOUT_EFFECTIVE);
        let name = keymap.layout_get_name(group).to_string();
        let chinese = self.cfg.is_chinese_layout(&name);
        log::debug!("layout {group}: {name:?} -> chinese={chinese}");
        self.set_enabled(chinese);
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
                let layouts: Vec<String> = (0..keymap.num_layouts())
                    .map(|i| keymap.layout_get_name(i).to_string())
                    .collect();
                log::debug!("keymap loaded ({size} bytes), layouts {layouts:?}");
                self.follow_layout = self.cfg.follow_layout && layouts.len() > 1;
                if self.cfg.follow_layout && layouts.len() == 1 {
                    log::info!(
                        "only one keyboard layout configured; add a Chinese input source in \
                         COSMIC Settings > Keyboard to switch with Super+Space"
                    );
                }
                self.xkb_state = Some(xkb::State::new(&keymap));
                self.keymap = Some(keymap);
                self.virtual_keyboard.keymap(format as u32, vk_fd.as_fd(), size);
                self.keymap_fd = Some(vk_fd);
                self.apply_layout();
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

        let Some(xkb_state) = self.xkb_state.as_ref() else {
            self.forward_key(time, key, key_state);
            return;
        };
        let input = keys::translate(xkb_state, keycode);
        let is_shift = keys::is_shift(keys::keysym(xkb_state, keycode));
        let is_modifier = matches!(input.key, Key::Modifier(_));

        if key_state == KeyState::Released {
            if self.repeat.is_some_and(|(k, _)| k == key) {
                self.stop_repeat();
            }
            if is_shift && self.shift_tap_pending {
                self.shift_tap_pending = false;
                self.forward_key(time, key, key_state);
                self.toggle_enabled();
                return;
            }
            let swallowed = self.consumed_keys.remove(&key);
            if self.enabled {
                let response = self.engine.release_key(input);
                self.apply_response(response, time, key, key_state, swallowed, None);
            } else if !swallowed {
                self.forward_key(time, key, key_state);
            }
            return;
        }

        log::trace!("key {key} -> {input:?}");
        self.stop_repeat();
        self.shift_tap_pending = self.cfg.shift_tap_toggle
            && is_shift
            && !input.modifiers.ctrl
            && !input.modifiers.alt
            && !input.modifiers.logo;

        if self.cfg.ctrl_space_toggle && input.key == Key::Space && input.modifiers.ctrl {
            self.consumed_keys.insert(key);
            self.toggle_enabled();
            return;
        }

        if !self.enabled {
            self.forward_key(time, key, key_state);
            return;
        }

        let response = self.engine.process_key(input);
        // Modifier keys always reach the application, whatever the engine
        // did with them, so its modifier state matches ours.
        if is_modifier {
            if response != Response::Ignored {
                let commit = match &response {
                    Response::Commit(t) | Response::CommitAndForward(t) => Some(t.clone()),
                    _ => None,
                };
                self.sync_to_client(commit.as_deref());
            }
            self.forward_key(time, key, key_state);
            return;
        }
        self.apply_response(response, time, key, key_state, false, Some(input));
    }

    /// Act on an engine response for `key`. Presses the engine consumed are
    /// swallowed (and so is their release later); releases are forwarded
    /// whenever their press was, whatever the engine did with them.
    fn apply_response(
        &mut self,
        response: Response,
        time: u32,
        key: u32,
        key_state: KeyState,
        swallowed: bool,
        repeat_input: Option<KeyInput>,
    ) {
        let (consumed, commit) = match response {
            Response::Ignored => (false, None),
            Response::Consumed | Response::Bell => (true, None),
            Response::Commit(text) => (true, Some(text)),
            Response::CommitAndForward(text) => (false, Some(text)),
        };
        if consumed || commit.is_some() {
            self.sync_to_client(commit.as_deref());
        }
        match key_state {
            KeyState::Pressed if consumed => {
                self.consumed_keys.insert(key);
                if let Some(input) = repeat_input {
                    self.start_repeat(key, input);
                }
            }
            KeyState::Pressed => self.forward_key(time, key, key_state),
            _ if swallowed => {}
            _ => self.forward_key(time, key, key_state),
        }
    }

    /// Push the engine's visible state (and an optional commit) to the text
    /// field, and refresh the candidate popup.
    fn sync_to_client(&mut self, commit: Option<&str>) {
        if let Some(text) = commit {
            log::debug!("commit {text:?}");
            self.input_method.commit_string(text.to_string());
        }
        let preedit = self.engine.preedit();
        let cursor = preedit.cursor as i32;
        self.input_method.set_preedit_string(preedit.text.clone(), cursor, cursor);
        self.input_method.commit(self.done_serial);

        let candidates = self.engine.candidates();
        if preedit.text.is_empty() && candidates.is_empty() {
            self.popup.hide();
        } else {
            let (page, selected) = (self.engine.page_info(), self.engine.selected());
            self.popup.show(&preedit.text, &candidates, page, selected);
        }
    }

    fn handle_done(&mut self) {
        self.done_serial = self.done_serial.wrapping_add(1);
        if self.pending_deactivate {
            self.pending_deactivate = false;
            log::debug!("text input deactivated");
            self.stop_repeat();
            self.drop_grab();
            self.release_forwarded_keys();
            self.engine.reset();
            self.consumed_keys.clear();
            self.popup.hide();
        }
        if self.pending_activate {
            self.pending_activate = false;
            log::debug!("text input activated");
            self.drop_grab();
            self.grab = Some(self.input_method.grab_keyboard(&self.qh, ()));
            self.engine.reset();
            self.consumed_keys.clear();
            self.popup.hide();
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
                state.apply_layout();
            }
            Event::RepeatInfo { rate, delay } => {
                state.repeat_rate = (rate > 0 && delay >= 0).then(|| {
                    (Duration::from_millis(delay as u64), Duration::from_millis((1000 / rate as u64).max(1)))
                });
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwpInputPopupSurfaceV2, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwpInputPopupSurfaceV2,
        event: zwp_input_popup_surface_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwp_input_popup_surface_v2::Event::TextInputRectangle { x, y, width, height } = event {
            log::trace!("text input rectangle {x},{y} {width}x{height}");
        }
    }
}

impl Dispatch<WpFractionalScaleV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            state.popup.set_scale(scale as f32 / 120.0);
        }
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_surface::WlSurface,
        event: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Integer fallback for compositors without wp_fractional_scale_v1.
        if let wl_surface::Event::PreferredBufferScale { factor } = event {
            state.popup.set_scale(factor as f32);
        }
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for State {
    fn event(
        _: &mut Self,
        buffer: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            buffer.destroy();
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

delegate_noop!(State: ignore wl_compositor::WlCompositor);
delegate_noop!(State: ignore wl_shm::WlShm);
delegate_noop!(State: ignore wl_shm_pool::WlShmPool);
delegate_noop!(State: ignore WpFractionalScaleManagerV1);
delegate_noop!(State: ignore WpViewporter);
delegate_noop!(State: ignore WpViewport);
delegate_noop!(State: ignore ZwpInputMethodManagerV2);
delegate_noop!(State: ignore ZwpVirtualKeyboardManagerV1);
delegate_noop!(State: ignore ZwpVirtualKeyboardV1);

impl Drop for State {
    fn drop(&mut self) {
        self.popup.hide();
        self.drop_grab();
        self.release_forwarded_keys();
        self.virtual_keyboard.destroy();
        self.input_method.destroy();
    }
}
