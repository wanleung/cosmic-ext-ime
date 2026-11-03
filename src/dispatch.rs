//! Key-dispatch decision policies of the input-method frontend, extracted
//! from the Wayland handlers into pure, unit-testable form:
//!
//! * [`disposition`] — what to do with an engine [`Response`] for a key
//!   press/release (swallow, forward, commit, drop the release of a
//!   swallowed key).
//! * [`ShiftTap`] — the bare-Shift-tap Chinese/English toggle state
//!   (`shift-tap-toggle` setting).  A Shift used as a chord modifier
//!   (Shift+, for ，) must cancel the pending tap; only a bare
//!   press+release of Shift toggles.

use cosmic_ext_ime_engine::Response;

/// What the frontend should do after an engine [`Response`] for a key event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    /// Swallow the press (its release will be dropped via [`Action::DropRelease`]).
    ConsumePress,
    /// Forward the key to the application untouched.
    ForwardPress,
    /// Release of a previously swallowed key: drop it.
    DropRelease,
    /// Release to forward to the application.
    ForwardRelease,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Disposition {
    /// Push visible state (preedit/candidates) and an optional commit to
    /// the text field.
    pub sync: bool,
    /// Commit text for the application.
    pub commit: Option<String>,
    pub action: Action,
}

pub(crate) fn disposition(response: &Response, pressed: bool, swallowed: bool) -> Disposition {
    let (sync, commit) = match response {
        Response::Ignored => (false, None),
        Response::Consumed | Response::Bell => (true, None),
        Response::Commit(t) => (true, Some(t.clone())),
        Response::CommitAndForward(t) => (true, Some(t.clone())),
    };
    let action = match (pressed, response) {
        (true, Response::Ignored) => Action::ForwardPress,
        (true, Response::CommitAndForward(_)) => Action::ForwardPress,
        (true, _) => Action::ConsumePress,
        (false, _) if swallowed => Action::DropRelease,
        (false, _) => Action::ForwardRelease,
    };
    Disposition {
        sync,
        commit,
        action,
    }
}

/// Bare-Shift-tap toggle state machine.
#[derive(Debug)]
pub(crate) struct ShiftTap {
    enabled: bool,
    pending: bool,
}

impl ShiftTap {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            pending: false,
        }
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Any key press.  `is_shift_tap_candidate` is true when the pressed key
    /// is a Shift key and no chord modifiers (Ctrl/Alt/Super) are held.
    pub(crate) fn on_press(&mut self, is_shift_tap_candidate: bool) {
        self.pending = self.enabled && is_shift_tap_candidate;
    }

    /// Shift key release.  Returns `true` when a bare Shift tap should
    /// toggle the Chinese/English mode.
    pub(crate) fn on_release(&mut self) -> bool {
        let toggle = self.enabled && self.pending;
        self.pending = false;
        toggle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignored_press_is_forwarded_without_sync() {
        let d = disposition(&Response::Ignored, true, false);
        assert_eq!(d.action, Action::ForwardPress);
        assert!(!d.sync);
        assert_eq!(d.commit, None);
    }

    #[test]
    fn consumed_press_is_swallowed_and_synced() {
        let d = disposition(&Response::Consumed, true, false);
        assert_eq!(d.action, Action::ConsumePress);
        assert!(d.sync);
        assert_eq!(d.commit, None);
    }

    #[test]
    fn bell_is_swallowed_with_error_flag() {
        let d = disposition(&Response::Bell, true, false);
        assert_eq!(d.action, Action::ConsumePress);
        assert!(d.sync);
    }

    #[test]
    fn commit_press_is_swallowed_with_text() {
        let d = disposition(&Response::Commit("你".into()), true, false);
        assert_eq!(d.action, Action::ConsumePress);
        assert!(d.sync);
        assert_eq!(d.commit, Some("你".to_string()));
    }

    /// CommitAndForward: the text reaches the application twice by design —
    /// as a commit AND as the forwarded key (e.g. tone keys).
    #[test]
    fn commit_and_forward_press_is_forwarded_with_text() {
        let d = disposition(&Response::CommitAndForward("，".into()), true, false);
        assert_eq!(d.action, Action::ForwardPress);
        assert!(d.sync);
        assert_eq!(d.commit, Some("，".to_string()));
    }

    /// The release of a swallowed key must be dropped, never forwarded —
    /// otherwise the application would see half a key chord.
    #[test]
    fn release_of_swallowed_key_is_dropped() {
        let d = disposition(&Response::Consumed, false, true);
        assert_eq!(d.action, Action::DropRelease);
        assert_eq!(d.commit, None);
    }

    /// A release the engine never swallowed is forwarded.
    #[test]
    fn release_of_forwarded_key_is_forwarded() {
        let d = disposition(&Response::Ignored, false, false);
        assert_eq!(d.action, Action::ForwardRelease);
        assert!(!d.sync);
    }

    /// Shift tap: disabled setting never toggles.
    #[test]
    fn shift_tap_disabled_never_toggles() {
        let mut t = ShiftTap::new(false);
        t.on_press(true);
        assert!(!t.on_release());
    }

    /// A bare Shift press+release toggles.
    #[test]
    fn shift_tap_bare_tap_toggles() {
        let mut t = ShiftTap::new(true);
        t.on_press(true);
        assert!(t.on_release());
        // And the tap is one-shot.
        assert!(!t.on_release());
    }

    /// Shift used as a chord modifier (Shift+, → ，) must NOT toggle:
    /// the non-Shift press cancels the pending tap.
    #[test]
    fn shift_chord_cancels_tap() {
        let mut t = ShiftTap::new(true);
        t.on_press(true); // Shift down
        t.on_press(false); // ',' down — cancels
        assert!(!t.on_release()); // Shift up — no toggle
    }

    /// Both Shifts pressed together still count as one tap.
    #[test]
    fn shift_tap_both_shifts() {
        let mut t = ShiftTap::new(true);
        t.on_press(true);
        t.on_press(true); // the other Shift
        assert!(t.on_release());
    }
}
