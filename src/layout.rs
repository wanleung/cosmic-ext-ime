//! The layout-following policy of the input-method frontend, kept out of the
//! Wayland handlers so it can be unit-tested.
//!
//! The invariant (`NoDisableWhileChinese`: the input method must never
//! disable itself while the user types Chinese) is enforced by evaluating
//! ONLY settled modifiers reports, the ones sent once every key is released
//! (`depressed == 0 && latched == 0`).  Settled reports carry the true input
//! source and never flap during typing.  Reports taken with a modifier held
//! used to carry a transitional group 0, because the compositor rebuilt its
//! xkb state on every keymap swap (docs/compositor-notes.md §2); sharing the
//! seat keymap removed that, but the policy costs nothing and is the one
//! TLC verifies (`docs/tla/IMELayoutFixed.tla`).
//!
//! A keymap load resets the sync and never evaluates by itself (a fresh
//! xkb state always reports group 0, the `KeymapReload` counterexample);
//! the compositor follows every load with one settled report that syncs.

use cosmic_ext_ime_config::PopeinputConfig;

pub(crate) struct LayoutPolicy {
    /// Group of the last evaluation.  `None` until a deliberate switch has
    /// synced the policy to the compositor's group numbering.
    last_group: Option<u32>,
}

impl LayoutPolicy {
    pub(crate) fn new() -> Self {
        Self { last_group: None }
    }

    /// Keymap load: resets the group sync.  Never decides `enabled` — a
    /// fresh xkb state always reports group 0 and the compositor follows
    /// every load with a settled event that syncs.
    pub(crate) fn on_keymap_load(&mut self) {
        self.last_group = None;
    }

    /// A settled modifiers event (no key held).  Returns the `enabled`
    /// decision when the settled group differs from the last evaluated one.
    /// `group_name` is the keymap's layout name for `group`.
    pub(crate) fn on_settled_group(
        &mut self,
        group: u32,
        group_name: Option<&str>,
        cfg: &PopeinputConfig,
        follow_layout: bool,
    ) -> Option<bool> {
        if !follow_layout {
            return None;
        }
        if self.last_group == Some(group) {
            return None;
        }
        let name = group_name?;
        let chinese = cfg.is_chinese_layout(name);
        self.last_group = Some(group);
        Some(chinese)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_ext_ime_config::PopeinputConfig;

    fn cfg() -> PopeinputConfig {
        PopeinputConfig::default()
    }

    const EN: &str = "English (US)";
    const ZH: &str = "Chinese";

    /// Deliberate Super+Space round trip: the settled group tracks the real
    /// source (journal-verified trace), so the IM follows it.
    #[test]
    fn settled_group_switches_follow_the_source() {
        let mut p = LayoutPolicy::new();
        p.on_keymap_load();
        assert_eq!(p.on_settled_group(1, Some(ZH), &cfg(), true), Some(true));
        assert_eq!(p.on_settled_group(0, Some(EN), &cfg(), true), Some(false));
        assert_eq!(p.on_settled_group(1, Some(ZH), &cfg(), true), Some(true));
    }

    /// Re-reporting the same settled group is a no-op (dedupe).
    #[test]
    fn same_settled_group_is_skipped() {
        let mut p = LayoutPolicy::new();
        p.on_keymap_load();
        assert_eq!(p.on_settled_group(1, Some(ZH), &cfg(), true), Some(true));
        assert_eq!(p.on_settled_group(1, Some(ZH), &cfg(), true), None);
    }

    /// After a keymap load the sync resets: the next settled event decides
    /// again even for the same group (focus changes must not strand the IM
    /// in a stale state).
    #[test]
    fn keymap_load_resyncs() {
        let mut p = LayoutPolicy::new();
        p.on_keymap_load();
        assert_eq!(p.on_settled_group(1, Some(ZH), &cfg(), true), Some(true));
        p.on_keymap_load();
        assert_eq!(p.on_settled_group(1, Some(ZH), &cfg(), true), Some(true));
    }

    /// The "any key switches to English" regression: transitional events
    /// (modifiers held — e.g. a bare Super tap for the launcher) never
    /// reach this policy; the caller gates on depressed == 0.
    #[test]
    fn transitional_events_are_caller_gated() {
        let mut p = LayoutPolicy::new();
        p.on_keymap_load();
        // Settled on Chinese...
        assert_eq!(p.on_settled_group(1, Some(ZH), &cfg(), true), Some(true));
        // ...a Super-press report (group 0) must be filtered by the caller
        // before it can ever reach this method.
        assert_eq!(p.last_group, Some(1));
    }

    /// Without follow_layout nothing ever decides.
    #[test]
    fn no_follow_layout_no_decision() {
        let mut p = LayoutPolicy::new();
        p.on_keymap_load();
        assert_eq!(p.on_settled_group(1, Some(ZH), &cfg(), false), None);
        assert_eq!(p.on_settled_group(0, Some(EN), &cfg(), false), None);
    }

    /// An unusable group name defers the decision instead of guessing.
    #[test]
    fn unknown_group_name_defers() {
        let mut p = LayoutPolicy::new();
        p.on_keymap_load();
        assert_eq!(p.on_settled_group(7, None, &cfg(), true), None);
    }
}
