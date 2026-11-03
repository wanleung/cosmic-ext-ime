# Compositor notes: how keys reach the input method on COSMIC

Everything here was read from the sources of the installed compositor
(cosmic-comp 8cb1802, smithay 8a27023) and confirmed with journal traces and
the probe tools in `tools/`.  It explains the frontend decisions in `src/im.rs`.

## 1. The pipeline

1. A physical key updates the seat's xkb state (`KeyboardHandle::input` →
   `key_input`), runs the compositor's shortcut filter, and is then handed to
   the active keyboard grab.  With a text field focused that grab is ours
   (`zwp_input_method_keyboard_grab_v2`): smithay sends the `key` event and,
   *after* it, a `modifiers` event when the modifiers changed
   (`input_method_keyboard_grab.rs`).  The `group` field is the **effective**
   layout, not the locked one.
2. Keys the engine does not want are forwarded through our
   `zwp_virtual_keyboard_v1`.  cosmic-comp recognises the grab client's
   virtual keyboard and delivers those keys to the focused application
   directly (`input_forward_bypassing_grab`), with a `modifiers` event
   carrying the seat's current state — sent *after* the key
   (`KeyboardInnerHandle::input`).
3. Text goes to the application through `zwp_input_method_v2`
   `commit_string` / `set_preedit_string`; the candidate window is a
   `zwp_input_popup_surface_v2`.

## 2. Share the seat keymap (the input-source bug)

A virtual keyboard that sends its own keymap makes cosmic-comp swap the
seat keymap on every alternation between a physical key and a forwarded
one (`update_virtual_keyboard_keymap` runs on every key event), and every
swap rebuilds the xkb state from scratch (smithay `update_xkb_state`: a
fresh `xkb::State`, only pressed keys replayed).  The locked layout went back
to group 0 and Caps Lock was dropped mid-typing.  That was the whole family
of "input source randomly switches to English" reports: the "transitional
group 0 while a modifier is held" was never an xkb property.

The frontend therefore sends its keymap with the `no_keymap` format and
shares the seat's keymap and state.  `COSMIC_EXT_IME_VK_OWN_KEYMAP=1`
restores the old behaviour for compositors that need a separate keymap.

## 3. No modifiers echo

With the state shared, the seat already holds the modifiers it just
reported.  Echoing them through `modifiers` lands later (after a Shift may
have been released) and, because the reported `group` is the effective
layout, could pin a stale locked layout through `set_modifier_mask`.  The
frontend only sends `modifiers` to clear Caps Lock (below) or in own-keymap
mode.

## 4. Layout reports and the settled policy

Chinese input follows the input source (`follow_layout`).  Decisions are
taken from **settled** modifier reports only (`depressed == 0 && latched ==
0`), never from a keymap load (a fresh state reports group 0 on every focus
change).  `src/layout.rs` holds the policy; `docs/tla/IMELayoutFixed.tla`
is the TLC-verified model and `IMELayoutCurrent.tla` the violating one.
With the keymap shared this policy is no longer load-bearing, but it costs
nothing and is proven.

## 5. Caps Lock

The compositor toggles its Lock modifier before the grab sees the key.
While Chinese input is on, Caps Lock is only the Chinese/English toggle: the
engine toggles on the key, the key is swallowed, and once it is up the
frontend clears the Lock again through the shared state (English sub-mode
types lower case, Shift for capitals, LED off).  An ibus-style "mode follows
the LED" rule was tried first and made the two switches fight: a Caps Lock
pressed on the English source left every later switch to the Chinese source
in upper-case English.  A switch to the Chinese source and every focus into
a text field reset the engine to Chinese (`InputEngine::set_english_mode`),
because the sub-mode has no indicator and must not persist silently.  With
Chinese input off, Caps Lock is a plain Caps Lock for the application.

xkb keeps Lock in the depressed set while the key is held and an unlocking
press clears the lock only on release, so the lock state is read once the
key is up.

## 6. Verifying without a user

- `tools/vktap`: a `zwp_virtual_keyboard_v1` client (shared keymap) that taps
  evdev keycodes or `a+b` chords through the FULL compositor pipeline (seat
  state, shortcuts, the grab).  `VKTAP_DELAY_MS`, `VKTAP_GAP_MS`,
  `VKTAP_FOCUS_FILE` (abort unless the file reads `1`).
- `tools/improbe`: a text-field window that draws what an application
  receives — forwarded keys resolved through its own xkb state, text-input
  commits, the live preedit — and logs every event.  `PROBE_FOCUS_FILE`
  tracks its keyboard focus.
- `tools/run_line.sh`: runs one line through both and compares;
  `tools/gen_mixed_line.py` builds an 88-word English/Chinese sentence as
  keystrokes (standard Zhuyin layout, Caps Lock between segments).
- Journal: `journalctl --user -u 'app-io.github.wanleung.CosmicExtIme@autostart.service'`
  with `RUST_LOG=debug` logs every modifiers report, layout decision,
  commit and Caps Lock clear; `trace` adds every key.  A settled Chinese
  source reads `depressed=0x00000000 ... group=1`.
- `CHEWING_KEYMAP_DUMP=/path/file` makes the frontend write every keymap it
  receives from the compositor to that file, for checking what a layout
  really maps.
- The fresh binary can run in place of the service without root:
  `systemctl --user stop app-…@autostart.service` and start
  `target/release/cosmic-ext-ime` with `RUST_LOG=debug`.

Every behavioural claim about the compositor must come from source or a
trace; three consecutive "fixes" failed on unverified assumptions about the
`group` field.  Ported tests must port their preconditions too.

## 7. Worth reporting upstream

- smithay `update_xkb_state` drops the locked layout and lock modifiers on
  a keymap swap.
- cosmic-comp swaps keymaps per key for a virtual keyboard whose keymap
  equals the seat's.
- smithay sends a forwarded key before the modifiers event it carries, so
  the first key after a modifier change resolves with stale modifiers.

## 8. Parity backlog

- `default-use-english-mode`, `default-english-case`, mode-change toast,
  Shift+special-key forwarding, `use-system-keyboard-layout` remap
- candidate popup: labels from the configured selection keys, mouse
  selection
- frontend integration harness (headless compositor); the Wayland loop
  itself is not unit-tested
