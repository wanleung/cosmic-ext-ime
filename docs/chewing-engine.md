# Chewing engine for cosmic-ext-ime — deliverable

## Goal

Add a first-class **Chewing (新酷音 / libchewing)** engine to `cosmic-ext-ime`, selectable in the existing Input Method Settings, so COSMIC users can type Traditional Chinese with Zhuyin using the same library as `ibus-chewing`.

Target platform: Pop!_OS 24.04 / Ubuntu 24.04 (noble), **libchewing 0.6.0**.

## Deliverables

### 1. `crates/chewing-sys`

Raw FFI bindings generated with `bindgen` against `libchewing3-dev`:

- `build.rs` uses `pkg-config` (`chewing`) and links `libchewing`.
- Exposes `ChewingContext`, `chewing_new2`, `chewing_delete`, `chewing_free`,
  keyboard/mode setters, key handlers, buffer/aux/commit strings, candidate
  enumeration/selection, page info, reset, and user-dictionary APIs.
- Builds on Ubuntu 24.04 without vendoring libchewing.

### 2. Cargo package `cosmic-ext-ime-chewing`

Safe Rust adapter implementing `cosmic_ext_ime_engine::InputEngine`. It is wired
exactly like `cosmic-ext-ime-pinyin`: a separate Cargo package, compiled into
the daemon, with `libchewing3` as the runtime shared library. There is no
runtime plugin ABI and no `dlopen`.

- Key mapping: letters/digits/punctuation, Space, Backspace, Escape, Enter, Tab,
  arrows, PageUp/PageDown, Shift.
- Preedit from `chewing_buffer_String`; auxiliary/header from `chewing_aux_String`.
- Candidates from `chewing_cand_Enumerate` / `chewing_cand_String`, current page only.
- Selection commits through `chewing_commit_preedit_buf` when a numbered
  candidate is chosen.
- Commit strings are only read when `chewing_commit_Check` reports a new commit;
  a commit is reported whenever libchewing's `chewing_commit_Check` says one is pending.
- Page info from `chewing_cand_CurrentPage`, `TotalPage`, `ChoicePerPage`.
- Reset clears the composition and candidate list.
- Config: keyboard type, candidates per page, space-as-selection, auto-learn,
  maximum Chinese symbol length, and user dictionary file.
- User dictionary: if `~/.chewing/chewing.sqlite3` exists (the ibus-chewing
  dictionary on Ubuntu 24.04), libchewing 0.6 opens it directly; otherwise a new
  `~/.local/share/cosmic-ext-ime/chewing/user.db` is created.
- `Drop` calls `chewing_delete`.

### 3. App integration

- New `Engine::Chewing` variant in `crates/config`.
- New `build_chewing()` in `src/config.rs`.
- New config fields:
  - `chewing_keyboard`
  - `chewing_space_as_selection`
  - `chewing_auto_learn`
- Engine fields included in `engine_fields()` so changes rebuild the engine.
- Input Method Settings UI: engine selector entry, keyboard dropdown, and the two
  toggles. Changes apply live through `cosmic-config`.
- Workspace `Cargo.toml` includes both new crates.

### 4. Packaging

- `debian/control`: `Build-Depends: libchewing3-dev`;
  `Depends: libchewing3, libchewing3-data`.
- The engine is compiled into the `cosmic-ext-ime` binary, like libpinyin.
  The only separately installed runtime package is `libchewing3`, analogous to
  `libpinyin15`; there is no separately installable Debian engine package.
- README/settings text updated to mention Chewing.

### 5. Tests

- `crates/chewing/tests/engine.rs`:
  - initializes libchewing with a temporary user dictionary;
  - types a Zhuyin sequence, selects a candidate, and checks the committed text;
  - checks preedit/candidates/page info and reset.
- `cargo test -p cosmic-ext-ime-chewing`.
- `cargo check` for the workspace.

### 6. Caps Lock and the virtual keyboard (frontend)

Verified against the installed cosmic-comp (commit 8cb1802, smithay 8a27023)
by reading the compositor source; every claim below has a journal trace.

- **One xkb state, shared.** The frontend's `zwp_virtual_keyboard_v1` sends
  its keymap with the `no_keymap` format, so it shares the seat's keymap and
  xkb state.  A virtual keyboard with a keymap of its own makes cosmic-comp
  swap the seat keymap on every alternation between a physical key and a
  forwarded one (`update_virtual_keyboard_keymap`), and every swap rebuilds
  the xkb state from scratch (smithay `update_xkb_state`): the locked layout
  went back to group 0 and Caps Lock was dropped mid-typing.  That was the
  whole "input source randomly switches to English" family of bugs.
  `COSMIC_EXT_IME_VK_OWN_KEYMAP=1` restores the old behaviour (own keymap +
  modifiers echo) for compositors that need it.
- **No modifiers echo.** With the state shared, the seat already holds the
  modifiers it just reported; echoing them through `modifiers` lands later
  (after a Shift may have been released) and, since the reported `group` is
  the *effective* layout, could pin a stale locked layout via
  `set_modifier_mask`.  Forwarded keys reach the application with the seat's
  own modifier state (`input_forward_bypassing_grab(.., mods_changed=true)`).
- **Caps Lock is a pure Chinese/English toggle while Chinese input is on.**
  The compositor toggles its Lock modifier before the grab sees the key.
  The engine toggles on the key (ibus-chewing parity), the key is swallowed,
  and once the key is up the frontend clears the Lock again through the
  shared seat state (`modifiers` → cosmic-comp `set_modifier_mask`), so
  English sub-mode types lower case (Shift for capitals) and the LED stays
  off.  An ibus-style "mode follows the LED" rule was tried first and made
  the two switches fight: a Caps Lock pressed on the English source left
  every later switch to the Chinese source in upper-case English.  A switch
  to the Chinese source, and every focus into a text field, resets the
  engine to Chinese (`InputEngine::set_english_mode(false)`): the sub-mode
  has no indicator, so it never persists silently across fields.  With Chinese input off (English
  source, or Ctrl+Space), Caps Lock is a plain Caps Lock for the
  application.

### 7. Plain Zhuyin (傳統注音, `chewing_plain_zhuyin`)

ibus-chewing 2.0 offers `conversion-engine=simple` (libchewing ≥ 0.9
`SimpleEngine`); libchewing 0.6 has no such engine, so the adapter emulates
it, following libchewing master's state machine (`editor/mod.rs`):

- a completed syllable opens the candidate window for that character at
  once (`chewing_cand_open`); the preedit shows the first candidate;
- a selection key picks and commits the character (libchewing commits
  whenever SimpleEngine returns to the entering state), Enter commits the
  highlighted one, Escape throws the character away;
- any other key while the window is open accepts the current character and
  is processed as the start of the next syllable (libchewing retries the key
  as syllable input).  Digits select while the window is open — including
  ㄅㄉㄓㄚㄞㄢ — exactly as upstream;
- the buffer never holds more than one character, so no phrase conversion
  ever rewrites what was chosen.

## Acceptance criteria

- In Input Method Settings, **新酷音 (libchewing)** appears in the engine list.
- Selecting it and typing Zhuyin produces a preedit and candidate list.
- Number keys select candidates and commit text.
- Backspace/Escape/Enter/paging behave like Chewing.
- Keyboard type, space-as-selection and auto-learn persist and take effect.
- `cargo test -p cosmic-ext-ime-chewing` passes on Ubuntu 24.04 with
  `libchewing3-dev` installed.
- The app builds on Pop!_OS 24.04 with the packaged dependencies.

## Out of scope for this change

- Upstream merge into `wanleung/cosmic-ext-ime` (separate PR).
- Support for libchewing ≥ 0.11/0.13 APIs.
- X11/XWayland or Qt5 input support.
- Exact pixel/UI parity with ibus-chewing.
- New Wayland input-method frontend work; the existing COSMIC frontend is reused.
- User-dictionary migration from `~/.chewing` / ibus-chewing.
