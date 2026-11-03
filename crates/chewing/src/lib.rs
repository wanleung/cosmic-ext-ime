//! Chewing (新酷音) input engine backed by libchewing 0.6, the library behind
//! ibus-chewing.  The Wayland frontend only sees the frontend-agnostic
//! [`InputEngine`] trait; libchewing keeps the Zhuyin composition state.
//!
//! libchewing keeps an in-progress syllable in its bopomofo buffer, separate
//! from `chewing_buffer_String` (completed text).  Both must be part of the
//! preedit and of [`InputEngine::is_composing`], otherwise the first keys of a
//! syllable are forwarded to the application.
//!
//! The candidate window only opens when the user asks for it (Down, Space or
//! the backtick symbol key), exactly like ibus-chewing: libchewing keeps its
//! own selection mode open while the window shows, and keys that do not
//! select a candidate are absorbed until the window is dismissed with
//! Backspace or Escape.  [`Config::plain_zhuyin`] is the exception: there
//! every completed syllable opens the window.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;
use std::ptr;

use chewing_sys::ChewingContext;
use cosmic_ext_ime_engine::{Candidate, InputEngine, Key, KeyInput, PageInfo, Preedit, Response};

/// `chewing_set_ChiEngMode` values (libchewing `global.h`).
const CHINESE_MODE: c_int = chewing_sys::CHINESE_MODE as c_int;
const SYMBOL_MODE: c_int = chewing_sys::SYMBOL_MODE as c_int;
/// Size of libchewing's selection-key array, and so the largest page.
const MAX_SELKEY: usize = chewing_sys::MAX_SELKEY as usize;

/// libchewing keyboard layouts exposed by `chewing_KBStr2Num()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Keyboard {
    /// Standard Zhuyin (大千式), libchewing's default.
    #[default]
    Default,
    Hsu,
    Ibm,
    GinYieh,
    Et,
    Et26,
    Dvorak,
    DvorakHsu,
    DachenCp26,
}

impl Keyboard {
    /// Name accepted by `chewing_KBStr2Num()`.
    pub fn name(self) -> &'static CStr {
        match self {
            Keyboard::Default => c"KB_DEFAULT",
            Keyboard::Hsu => c"KB_HSU",
            Keyboard::Ibm => c"KB_IBM",
            Keyboard::GinYieh => c"KB_GIN_YIEH",
            Keyboard::Et => c"KB_ET",
            Keyboard::Et26 => c"KB_ET26",
            Keyboard::Dvorak => c"KB_DVORAK",
            Keyboard::DvorakHsu => c"KB_DVORAK_HSU",
            Keyboard::DachenCp26 => c"KB_DACHEN_CP26",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub keyboard: Keyboard,
    /// Candidates per page (1–10), further capped at the number of
    /// selection keys.
    pub page_size: usize,
    /// Let Space choose the highlighted candidate instead of paging.
    pub space_as_selection: bool,
    /// Learn new phrases into the user dictionary.
    pub auto_learn: bool,
    /// Maximum number of Chinese symbols in one composition.
    pub max_chi_symbol_len: usize,
    /// Full-width letters/punctuation (libchewing shape mode).
    pub fullwidth_chars: bool,
    /// Selection keys, in page order.  Only the first 10 ASCII characters
    /// count.  Default is "1234567890".
    pub selection_keys: String,
    /// ibus-chewing "easy symbol input" mode.
    pub easy_symbol_input: bool,
    /// Move the cursor after candidate selection.
    pub auto_shift_cur: bool,
    /// Add phrases in the forward direction.
    pub add_phrase_direction: bool,
    /// Prefer phrases to the rear of the cursor.
    pub phrase_choice_from_last: bool,
    /// Escape clears the whole preedit buffer.
    pub esc_clean_all_buf: bool,
    /// Plain Zhuyin (傳統注音): no automatic phrase conversion.  Every
    /// completed syllable opens the candidate window for that one
    /// character, a selection key commits it, and typing the next syllable
    /// accepts the current character.  Emulates ibus-chewing's
    /// `conversion-engine=simple` (libchewing `SimpleEngine`, which
    /// libchewing 0.6 does not have).
    pub plain_zhuyin: bool,
    /// libchewing's system data directory (`/usr/share/libchewing`).
    pub system_data_dir: PathBuf,
    /// User dictionary file.  libchewing creates it on first use; the parent
    /// directory is created by [`ChewingEngine::new`].
    pub user_data_file: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."));
        // ibus-chewing on Ubuntu 24.04 keeps its SQLite user dictionary here.
        // libchewing 0.6 can open it directly, so existing learned phrases are
        // reused instead of starting with an empty dictionary.
        let legacy_user = home
            .as_ref()
            .map(|h| h.join(".chewing").join("chewing.sqlite3"))
            .filter(|p| p.is_file());
        Config {
            keyboard: Keyboard::Default,
            page_size: 10,
            space_as_selection: true,
            auto_learn: true,
            max_chi_symbol_len: 20,
            fullwidth_chars: true,
            selection_keys: "1234567890".into(),
            easy_symbol_input: true,
            auto_shift_cur: true,
            add_phrase_direction: true,
            phrase_choice_from_last: true,
            esc_clean_all_buf: false,
            plain_zhuyin: false,
            system_data_dir: PathBuf::from(chewing_sys::PKGDATADIR),
            user_data_file: legacy_user.unwrap_or_else(|| {
                data_home
                    .join("cosmic-ext-ime")
                    .join("chewing")
                    .join("user.db")
            }),
        }
    }
}

/// A libchewing context plus the little state libchewing does not keep.
///
/// Invariant: `ctx` is the non-null context created by `chewing_new2` in
/// [`ChewingEngine::new`] and is freed only by `Drop`.  Every FFI call in
/// this file relies on it being live.
pub struct ChewingEngine {
    ctx: *mut ChewingContext,
    cfg: Config,
    /// Highlighted candidate within libchewing's current candidate page.
    /// libchewing has no highlight of its own; ibus-chewing tracks it in the
    /// lookup table, we track it here.
    cursor: usize,
}

/// A libchewing entry point that takes only the context (the
/// `chewing_handle_*` key handlers, `chewing_cand_open`, ...).
type CtxFn = unsafe extern "C" fn(*mut ChewingContext) -> c_int;
/// A libchewing key handler that takes the context and a key code.
type CtxKeyFn = unsafe extern "C" fn(*mut ChewingContext, c_int) -> c_int;

impl ChewingEngine {
    pub fn new(cfg: Config) -> Result<Self, String> {
        let parent = cfg
            .user_data_file
            .parent()
            .ok_or_else(|| "chewing user dictionary has no parent directory".to_string())?;
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("creating {}: {e}", parent.display()))?;

        let system = CString::new(cfg.system_data_dir.to_string_lossy().as_bytes())
            .map_err(|_| "chewing system data path contains NUL".to_string())?;
        let user = CString::new(cfg.user_data_file.to_string_lossy().as_bytes())
            .map_err(|_| "chewing user dictionary path contains NUL".to_string())?;

        // SAFETY: both paths are valid NUL-terminated strings that live for
        // the duration of the call; libchewing copies them.  A NULL logger is
        // accepted by libchewing 0.6.
        let ctx = unsafe {
            chewing_sys::chewing_new2(system.as_ptr(), user.as_ptr(), None, ptr::null_mut())
        };
        if ctx.is_null() {
            return Err(format!(
                "chewing_new2 failed (system {}, user {})",
                cfg.system_data_dir.display(),
                cfg.user_data_file.display()
            ));
        }

        let engine = ChewingEngine {
            ctx,
            cfg,
            cursor: 0,
        };
        engine.apply_config();
        log::info!(
            "libchewing initialised (system {}, user {})",
            engine.cfg.system_data_dir.display(),
            engine.cfg.user_data_file.display()
        );
        Ok(engine)
    }

    fn apply_config(&self) {
        let cfg = &self.cfg;
        let flag = |on: bool| c_int::from(on);
        // libchewing 0.6 uses 0 = enabled and 1 = disabled.
        let auto_learn = if cfg.auto_learn {
            chewing_sys::AUTOLEARN_ENABLED
        } else {
            chewing_sys::AUTOLEARN_DISABLED
        } as c_int;
        let sel_keys: Vec<c_int> = self.selection_keys().collect();
        // SAFETY: live context.  The keyboard name is a NUL-terminated
        // static string; `sel_keys` holds `sel_keys.len()` (at most
        // MAX_SELKEY) ints and libchewing copies them.
        unsafe {
            let kb_type = chewing_sys::chewing_KBStr2Num(cfg.keyboard.name().as_ptr());
            if kb_type >= 0 {
                chewing_sys::chewing_set_KBType(self.ctx, kb_type);
            }
            chewing_sys::chewing_set_ChiEngMode(self.ctx, CHINESE_MODE);
            chewing_sys::chewing_set_candPerPage(self.ctx, self.effective_page_size() as c_int);
            chewing_sys::chewing_set_spaceAsSelection(self.ctx, flag(cfg.space_as_selection));
            chewing_sys::chewing_set_autoLearn(self.ctx, auto_learn);
            chewing_sys::chewing_set_maxChiSymbolLen(
                self.ctx,
                cfg.max_chi_symbol_len.max(1) as c_int,
            );
            chewing_sys::chewing_set_easySymbolInput(self.ctx, flag(cfg.easy_symbol_input));
            chewing_sys::chewing_set_autoShiftCur(self.ctx, flag(cfg.auto_shift_cur));
            chewing_sys::chewing_set_addPhraseDirection(self.ctx, flag(cfg.add_phrase_direction));
            chewing_sys::chewing_set_phraseChoiceRearward(
                self.ctx,
                flag(cfg.phrase_choice_from_last),
            );
            chewing_sys::chewing_set_escCleanAllBuf(self.ctx, flag(cfg.esc_clean_all_buf));
            chewing_sys::chewing_set_ShapeMode(self.ctx, flag(cfg.fullwidth_chars));
            if !sel_keys.is_empty() {
                chewing_sys::chewing_set_selKey(
                    self.ctx,
                    sel_keys.as_ptr(),
                    sel_keys.len() as c_int,
                );
            }
        }
    }

    /// The configured selection keys libchewing can use: the first
    /// MAX_SELKEY ASCII characters.
    fn selection_keys(&self) -> impl Iterator<Item = c_int> + '_ {
        self.cfg
            .selection_keys
            .chars()
            .filter(|c| c.is_ascii())
            .take(MAX_SELKEY)
            .map(|c| c as c_int)
    }

    /// Candidates per page libchewing actually uses: the configured page size
    /// capped at the number of selection keys, like ibus-chewing's lookup
    /// table (ibus-chewing-lookup-table.c ibus_chewing_lookup_table_resize).
    fn effective_page_size(&self) -> usize {
        let sel_keys = self.selection_keys().count().max(1);
        self.cfg.page_size.clamp(1, MAX_SELKEY).min(sel_keys)
    }

    /// Call a libchewing function that takes only the context.  The return
    /// value is not a reliable "handled" flag (see [`Self::finish_key`]).
    fn call(&self, f: CtxFn) {
        // SAFETY: every `CtxFn` passed here is a libchewing entry point that
        // only needs a live context.
        unsafe { f(self.ctx) };
    }

    /// Call a libchewing key handler that takes the context and a key code.
    fn call_key(&self, f: CtxKeyFn, key: c_int) {
        // SAFETY: as for `call`; the key code is a plain int.
        unsafe { f(self.ctx, key) };
    }

    fn chi_eng_mode(&self) -> c_int {
        // SAFETY: live context.
        unsafe { chewing_sys::chewing_get_ChiEngMode(self.ctx) }
    }

    /// Switch libchewing's Chinese/English mode.  libchewing drops an
    /// in-progress syllable on every switch.
    fn set_chi_eng_mode(&self, mode: c_int) {
        // SAFETY: live context.
        unsafe { chewing_sys::chewing_set_ChiEngMode(self.ctx, mode) };
    }

    fn in_english_mode(&self) -> bool {
        self.chi_eng_mode() != CHINESE_MODE
    }

    fn full_shape(&self) -> bool {
        // SAFETY: live context.
        unsafe { chewing_sys::chewing_get_ShapeMode(self.ctx) != 0 }
    }

    /// Whether libchewing's candidate window is open right now.
    fn selection_open(&self) -> bool {
        // SAFETY: live context.  A non-zero total choice count is exactly
        // ibus-chewing's "table is showing" condition.
        unsafe { chewing_sys::chewing_cand_TotalChoice(self.ctx) > 0 }
    }

    fn current_page(&self) -> usize {
        // SAFETY: live context.
        unsafe { chewing_sys::chewing_cand_CurrentPage(self.ctx) }.max(0) as usize
    }

    fn total_pages(&self) -> usize {
        // SAFETY: live context.
        unsafe { chewing_sys::chewing_cand_TotalPage(self.ctx) }.max(0) as usize
    }

    /// Whether the open window has more than one candidate list (phrase
    /// lengths) to cycle through.
    fn has_other_cand_lists(&self) -> bool {
        // SAFETY: live context.
        unsafe {
            chewing_sys::chewing_cand_list_has_next(self.ctx) != 0
                || chewing_sys::chewing_cand_list_has_prev(self.ctx) != 0
        }
    }

    /// Candidates of libchewing's current page.  `chewing_cand_Enumerate`
    /// starts at the current page but would keep going through the following
    /// pages, so the iteration is capped at one page like ibus-chewing's
    /// lookup table.
    fn current_page_candidates(&self) -> Vec<Candidate> {
        if !self.selection_open() {
            return Vec::new();
        }
        let page_size = self.effective_page_size();
        let mut out = Vec::with_capacity(page_size);
        // SAFETY: live context.  The enumeration only walks libchewing's
        // already-populated choice list; every `chewing_cand_String` result
        // is a fresh allocation that `take_string` frees.
        unsafe {
            chewing_sys::chewing_cand_Enumerate(self.ctx);
            while out.len() < page_size && chewing_sys::chewing_cand_hasNext(self.ctx) != 0 {
                let text = take_string(chewing_sys::chewing_cand_String(self.ctx));
                if !text.is_empty() {
                    out.push(Candidate { text, hint: None });
                }
            }
        }
        out
    }

    fn page_len(&self) -> usize {
        self.current_page_candidates().len()
    }

    /// The selection key of the highlighted candidate.
    fn highlighted_selection_key(&self) -> c_int {
        let idx = self.cursor.min(self.page_len().saturating_sub(1));
        // SAFETY: live context.  `chewing_get_selKey` returns a malloc'd
        // copy of the MAX_SELKEY selection keys, or NULL; `idx` is below the
        // page size, which never exceeds MAX_SELKEY.  The copy is released
        // with `chewing_free`, like ibus-chewing does.
        unsafe {
            let keys = chewing_sys::chewing_get_selKey(self.ctx);
            if keys.is_null() {
                return '1' as c_int;
            }
            let key = *keys.add(idx);
            chewing_sys::chewing_free(keys.cast());
            key
        }
    }

    fn bopomofo_check(&self) -> bool {
        // SAFETY: live context.
        unsafe { chewing_sys::chewing_bopomofo_Check(self.ctx) != 0 }
    }

    /// The in-progress syllable.
    fn bopomofo(&self) -> String {
        // SAFETY: live context.  The `_static` variant returns a string owned
        // by libchewing (or NULL) that stays valid until the next call on the
        // context; it is copied out at once.
        unsafe {
            let p = chewing_sys::chewing_bopomofo_String_static(self.ctx);
            if p.is_null() {
                String::new()
            } else {
                CStr::from_ptr(p).to_string_lossy().into_owned()
            }
        }
    }

    /// The completed text of the composition.
    fn buffer(&self) -> String {
        // SAFETY: live context; the result is a fresh allocation or NULL.
        unsafe { take_string(chewing_sys::chewing_buffer_String(self.ctx)) }
    }

    fn buffer_len(&self) -> usize {
        self.buffer().chars().count()
    }

    /// libchewing's current commit string, whether or not the last key
    /// committed anything.
    fn take_commit(&self) -> String {
        // SAFETY: live context; the result is a fresh allocation or NULL.
        unsafe { take_string(chewing_sys::chewing_commit_String(self.ctx)) }
    }

    /// The text committed by the key just handled, or an empty string.
    /// `chewing_commit_Check` reports the last keystroke only, so the same
    /// commit is never read twice.
    fn take_new_commit(&self) -> String {
        // SAFETY: live context.
        if unsafe { chewing_sys::chewing_commit_Check(self.ctx) } != 0 {
            self.take_commit()
        } else {
            String::new()
        }
    }

    /// Page back in the open window like ibus-chewing's lookup table: on the
    /// first page, roll over into the next candidate list (libchewing's
    /// Down) when there is one.
    fn page_back(&self) {
        if self.current_page() == 0 && self.has_other_cand_lists() {
            self.call(chewing_sys::chewing_handle_Down);
        } else {
            self.call(chewing_sys::chewing_handle_PageUp);
        }
    }

    /// Page forward in the open window; past the last page, roll over into
    /// the next candidate list.
    fn page_forward(&self) {
        if self.current_page() + 1 >= self.total_pages() {
            self.call(chewing_sys::chewing_handle_Down);
        } else {
            self.call(chewing_sys::chewing_handle_PageDown);
        }
    }

    /// Run `f` with libchewing's easy-symbol table switched off, when `off`
    /// is set and the table is configured on.
    fn with_easy_symbols_off(&self, off: bool, f: impl FnOnce()) {
        let off = off && self.cfg.easy_symbol_input;
        // SAFETY (both blocks): live context.
        if off {
            unsafe { chewing_sys::chewing_set_easySymbolInput(self.ctx, 0) };
        }
        f();
        if off {
            unsafe { chewing_sys::chewing_set_easySymbolInput(self.ctx, 1) };
        }
    }

    /// Feed one key to libchewing.  Returns `false` for keys libchewing is
    /// not given at all.
    fn handle_key(&mut self, input: KeyInput) -> bool {
        use chewing_sys::{
            chewing_handle_Backspace, chewing_handle_Capslock, chewing_handle_Default,
            chewing_handle_Del, chewing_handle_Down, chewing_handle_End, chewing_handle_Enter,
            chewing_handle_Esc, chewing_handle_Home, chewing_handle_Left, chewing_handle_PageDown,
            chewing_handle_PageUp, chewing_handle_Right, chewing_handle_ShiftLeft,
            chewing_handle_ShiftRight, chewing_handle_ShiftSpace, chewing_handle_Space,
            chewing_handle_Tab, chewing_handle_Up,
        };

        let shift = input.modifiers.shift;
        let selecting = self.selection_open();
        match input.key {
            Key::Char(c) if c.is_ascii() => {
                // ibus self_key_sym_fix parity: in Chinese mode the letter
                // case comes from Shift alone, never from the keysym.  With
                // Caps Lock on, keysyms arrive uppercase and libchewing's
                // bopomofo tables only match lowercase.
                let c = if c.is_ascii_alphabetic() && !self.in_english_mode() {
                    if shift {
                        c.to_ascii_uppercase()
                    } else {
                        c.to_ascii_lowercase()
                    }
                } else {
                    c
                };
                // Easy symbols must not shadow bopomofo keys (ibus-chewing
                // disables them for unshifted keys); with the window open
                // libchewing only looks for selection keys anyway.
                self.with_easy_symbols_off(!shift && !selecting, || {
                    self.call_key(chewing_handle_Default, c as c_int)
                });
            }
            Key::Space if shift => self.call(chewing_handle_ShiftSpace),
            // Unshifted Space must not hit the easy-symbol table either, or an
            // idle Space would be absorbed instead of committing " ".
            // libchewing itself opens the window, pages forward, and rolls
            // over to the next candidate list on the last page.
            Key::Space => {
                self.with_easy_symbols_off(!selecting, || self.call(chewing_handle_Space));
            }
            // Backspace only closes the candidate window while it is open;
            // the composition is kept.
            Key::Backspace => self.call(chewing_handle_Backspace),
            Key::Escape => self.call(chewing_handle_Esc),
            // Enter on the open window selects the highlighted candidate by
            // sending its selection key, like ibus-chewing.
            Key::Enter if selecting => {
                let key = self.highlighted_selection_key();
                self.call_key(chewing_handle_Default, key);
            }
            Key::Enter => self.call(chewing_handle_Enter),
            Key::Tab => self.call(chewing_handle_Tab),
            Key::Up | Key::PageUp if selecting => self.page_back(),
            Key::Down | Key::PageDown if selecting => self.page_forward(),
            Key::Up => self.call(chewing_handle_Up),
            Key::Down => self.call(chewing_handle_Down),
            Key::PageUp => self.call(chewing_handle_PageUp),
            Key::PageDown => self.call(chewing_handle_PageDown),
            Key::Left if shift => self.call(chewing_handle_ShiftLeft),
            Key::Right if shift => self.call(chewing_handle_ShiftRight),
            // The horizontal lookup table moves the highlight within the page
            // with Left/Right and only pages at the edges.
            Key::Left if selecting => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                } else {
                    self.page_back();
                }
            }
            Key::Right if selecting => {
                if self.cursor + 1 < self.page_len() {
                    self.cursor += 1;
                } else {
                    self.page_forward();
                }
            }
            // Within the composition the arrows move the editing cursor
            // (ibus self_handle_left/right); with nothing composed they were
            // already forwarded by process_key.
            Key::Left => self.call(chewing_handle_Left),
            Key::Right => self.call(chewing_handle_Right),
            Key::Delete => self.call(chewing_handle_Del),
            Key::Home => self.call(chewing_handle_Home),
            Key::End => self.call(chewing_handle_End),
            Key::CapsLock => self.call(chewing_handle_Capslock),
            _ => return false,
        }
        true
    }

    /// The response to a key libchewing just processed.  `was_open` is
    /// whether the candidate window was open before it.
    fn finish_key(&mut self, handled: bool, was_open: bool) -> Response {
        if !handled {
            return Response::Ignored;
        }
        let commit = self.take_new_commit();
        self.cursor = if self.selection_open() {
            // A freshly opened window starts on the first candidate.
            let cursor = if was_open { self.cursor } else { 0 };
            cursor.min(self.page_len().saturating_sub(1))
        } else {
            0
        };
        if !commit.is_empty() {
            return Response::Commit(commit);
        }
        // libchewing reports KEYSTROKE_IGNORE through the output struct, not
        // the handle_* return value.
        // SAFETY: live context.
        let ignored = unsafe { chewing_sys::chewing_keystroke_CheckIgnore(self.ctx) != 0 };
        if ignored && !self.is_composing() {
            return Response::Ignored;
        }
        Response::Consumed
    }

    /// Ctrl+digit adds a user phrase (or opens the symbol list for 0/1).  In
    /// English sub-mode with nothing composed the key belongs to the
    /// application (ibus-chewing GitHub 144).
    fn ctrl_num(&mut self, digit: char) -> Response {
        if self.in_english_mode() && !self.is_composing() {
            return Response::Ignored;
        }
        let was_open = self.selection_open();
        self.call_key(chewing_sys::chewing_handle_CtrlNum, digit as c_int);
        self.finish_key(true, was_open)
    }

    /// A keypad key, as `c`, processed in temporary English (symbol) mode so
    /// it can never start a syllable (ibus-chewing
    /// self_handle_num_keypad).  Like ibus-chewing's wrapper, both mode
    /// switches clear an in-progress syllable.
    fn keypad_key(&mut self, input: KeyInput, c: char) -> Response {
        let was_open = self.selection_open();
        let mode = self.chi_eng_mode();
        self.set_chi_eng_mode(SYMBOL_MODE);
        let handled = self.handle_key(KeyInput {
            key: Key::Char(c),
            modifiers: input.modifiers,
        });
        self.set_chi_eng_mode(mode);
        self.finish_key(handled, was_open)
    }

    /// Plain Zhuyin (see [`Config::plain_zhuyin`]), following libchewing
    /// master's SimpleEngine (`editor/mod.rs`): it commits whenever it
    /// returns to the entering state and retries any other key as syllable
    /// input, with the same quirk that digits select while the window is
    /// open, even ㄅㄉㄓㄚㄞㄢ.  The buffer never holds more than the
    /// character being chosen.
    fn process_plain_zhuyin(&mut self, input: KeyInput) -> Response {
        let accepted = self.plain_accept_current(input);
        let was_open = self.selection_open();
        let syllable_before = self.bopomofo_check();
        let len_before = self.buffer_len();
        let handled = self.handle_key(input);
        let open = self.selection_open();
        let syllable_done =
            syllable_before && !self.bopomofo_check() && self.buffer_len() > len_before;
        if !was_open && !open && syllable_done {
            // The syllable became a character: let the user choose it.
            self.call(chewing_sys::chewing_cand_open);
        } else if was_open && !open && matches!(input.key, Key::Char(_) | Key::Enter) {
            // A candidate was picked: it is the committed text.
            self.call(chewing_sys::chewing_handle_Enter);
        } else if was_open && input.key == Key::Escape {
            // Escape throws the character away (libchewing SimpleEngine).
            self.call(chewing_sys::chewing_clean_preedit_buf);
        }
        prepend_commit(accepted, self.finish_key(handled, was_open))
    }

    /// Plain Zhuyin, before the key: accept the character on display.
    ///
    /// Anything but a selection key, or an Enter on a moved highlight,
    /// dismisses the window keeping the composition.  The preedit shows the
    /// conversion's choice, which need not be the list's first entry (ㄕˋ
    /// shows 是, the list starts with 市), so Enter must commit what is seen.
    /// A character key then commits an accepted character before starting
    /// the next syllable; that commit is returned.
    fn plain_accept_current(&mut self, input: KeyInput) -> String {
        let accept = self.selection_open()
            && match input.key {
                Key::Char(c) => input.modifiers.shift || !self.cfg.selection_keys.contains(c),
                Key::Enter => self.cursor == 0,
                _ => false,
            };
        if accept {
            self.call(chewing_sys::chewing_handle_Backspace);
        }
        if matches!(input.key, Key::Char(_))
            && !self.selection_open()
            && !self.buffer().is_empty()
            && !self.bopomofo_check()
        {
            self.call(chewing_sys::chewing_handle_Enter);
            return self.take_commit();
        }
        String::new()
    }
}

impl Drop for ChewingEngine {
    fn drop(&mut self) {
        // SAFETY: `ctx` came from `chewing_new2` and is never used again.
        unsafe { chewing_sys::chewing_delete(self.ctx) };
    }
}

/// Copy and release a string allocated by libchewing.
///
/// # Safety
///
/// `p` is NULL or a NUL-terminated string allocated by libchewing that
/// nothing else frees.
unsafe fn take_string(p: *mut c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    let s = CStr::from_ptr(p).to_string_lossy().into_owned();
    chewing_sys::chewing_free(p.cast());
    s
}

/// `committed` (text already taken from libchewing) in front of whatever
/// the key itself produced.
fn prepend_commit(committed: String, response: Response) -> Response {
    if committed.is_empty() {
        return response;
    }
    match response {
        Response::Commit(t) => Response::Commit(committed + &t),
        Response::CommitAndForward(t) => Response::CommitAndForward(committed + &t),
        _ => Response::Commit(committed),
    }
}

/// Keys that edit or navigate a composition.  With nothing composed
/// ibus-chewing forwards them all to the application.
fn is_editing_key(key: Key) -> bool {
    matches!(
        key,
        Key::Backspace
            | Key::Escape
            | Key::Enter
            | Key::Tab
            | Key::Up
            | Key::Down
            | Key::Left
            | Key::Right
            | Key::PageUp
            | Key::PageDown
            | Key::Delete
            | Key::Home
            | Key::End
    )
}

fn keypad_digit(d: u8) -> char {
    char::from(b'0' + d.min(9))
}

/// The character a keypad key types.
fn keypad_char(key: Key) -> Option<char> {
    match key {
        Key::KeypadDigit(d) => Some(keypad_digit(d)),
        Key::KeypadSymbol(c) => Some(c),
        _ => None,
    }
}

/// Full-width punctuation for keys that require Shift on a US layout,
/// pressed while no Chinese composition is active.  Unshifted punctuation
/// keys (`,` `.` `/` `;` `[` …) are Zhuyin keys in the Da Chen layout and
/// must reach libchewing as bopomofo, exactly like ibus-chewing.
fn fullwidth_punctuation(c: char) -> Option<&'static str> {
    // Characters and results follow libchewing's SpecialSymbolInput table
    // (src/chewingutil.c).  The backtick is deliberately absent: it opens the
    // symbol candidate menu instead of typing ｀.
    Some(match c {
        '<' => "，",
        '>' => "。",
        '?' => "？",
        '!' => "！",
        ':' => "：",
        '"' => "；",
        '{' => "『",
        '}' => "』",
        '(' => "（",
        ')' => "）",
        '_' => "—",
        '+' => "＋",
        '=' => "＝",
        '~' => "～",
        '@' => "＠",
        '#' => "＃",
        '$' => "＄",
        '%' => "％",
        '^' => "︿",
        '&' => "＆",
        '*' => "＊",
        '|' => "｜",
        '\\' => "＼",
        _ => return None,
    })
}

impl InputEngine for ChewingEngine {
    fn name(&self) -> &str {
        "新酷音"
    }

    fn process_key(&mut self, input: KeyInput) -> Response {
        let m = input.modifiers;
        let ctrl_only = m.ctrl && !m.alt && !m.logo;

        // ibus-chewing forwards Ctrl/Alt/Meta combinations to the
        // application, except Ctrl+digit (top row or keypad).
        match input.key {
            Key::Char(c) if ctrl_only && c.is_ascii_digit() => return self.ctrl_num(c),
            Key::KeypadDigit(d) if ctrl_only => return self.ctrl_num(keypad_digit(d)),
            _ if m.ctrl || m.alt || m.logo => return Response::Ignored,
            _ => {}
        }

        // An unshifted keypad key is forwarded unless something is composed
        // or full shape has to convert it (ibus self_handle_num_keypad).
        if let Some(c) = keypad_char(input.key) {
            if !m.shift && !self.full_shape() && !self.is_composing() {
                return Response::Ignored;
            }
            return self.keypad_key(input, c);
        }

        if !self.is_composing() {
            if is_editing_key(input.key) {
                return Response::Ignored;
            }
            if let Key::Char(c) = input.key {
                if self.in_english_mode() {
                    // ibus-chewing treats the English sub-mode like a
                    // disabled input method (GitHub 144): printable keys
                    // reach the application directly, unless full shape has
                    // to convert them.
                    if c.is_ascii() && !self.full_shape() {
                        return Response::Ignored;
                    }
                } else if let Some(punctuation) = fullwidth_punctuation(c) {
                    return Response::Commit(punctuation.to_string());
                }
            }
        }

        if self.cfg.plain_zhuyin && !self.in_english_mode() {
            return self.process_plain_zhuyin(input);
        }
        let was_open = self.selection_open();
        let handled = self.handle_key(input);
        self.finish_key(handled, was_open)
    }

    /// A mode switch drops an in-progress syllable (libchewing
    /// `set_language_mode` clears the bopomofo buffer, as the Caps Lock key
    /// itself does) and keeps the rest of the composition.
    fn set_english_mode(&mut self, english: bool) -> bool {
        if self.in_english_mode() == english {
            return false;
        }
        self.set_chi_eng_mode(if english { SYMBOL_MODE } else { CHINESE_MODE });
        true
    }

    fn preedit(&self) -> Preedit {
        let buffer = self.buffer();
        let bopomofo = self.bopomofo();
        if buffer.is_empty() && bopomofo.is_empty() {
            return Preedit::default();
        }
        // ibus inserts the bopomofo at the composition cursor and reports the
        // cursor after it (ibus-chewing-preedit.c ibus_chewing_pre_edit_update).
        // SAFETY: live context.
        let cursor_chars = unsafe { chewing_sys::chewing_cursor_Current(self.ctx) }.max(0) as usize;
        let chars: Vec<char> = buffer.chars().collect();
        let at = cursor_chars.min(chars.len());
        let mut text: String = chars[..at].iter().collect();
        let cursor = text.len() + bopomofo.len();
        text.push_str(&bopomofo);
        text.extend(&chars[at..]);
        Preedit { cursor, text }
    }

    fn candidates(&self) -> Vec<Candidate> {
        self.current_page_candidates()
    }

    fn page_info(&self) -> PageInfo {
        let total_pages = self.total_pages();
        let page = self.current_page().min(total_pages.saturating_sub(1));
        PageInfo {
            page,
            total_pages: Some(total_pages),
            has_next: page + 1 < total_pages,
            page_size: self.effective_page_size(),
        }
    }

    fn selected(&self) -> usize {
        self.cursor
    }

    fn reset(&mut self) {
        // Esc closes the window or drops the bopomofo, cand_close makes sure
        // selection mode is left, and clean_preedit_buf empties the buffer.
        self.call(chewing_sys::chewing_handle_Esc);
        self.call(chewing_sys::chewing_cand_close);
        self.call(chewing_sys::chewing_clean_preedit_buf);
        self.cursor = 0;
    }

    fn is_composing(&self) -> bool {
        self.bopomofo_check() || !self.buffer().is_empty() || self.selection_open()
    }

    fn header(&self) -> String {
        self.buffer() + &self.bopomofo()
    }
}
