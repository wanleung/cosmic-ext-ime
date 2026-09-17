//! Smart Pinyin (智能拼音) engine backed by libpinyin, the library behind
//! GNOME's "Intelligent Pinyin": sentence-level prediction, word-by-word
//! selection, fuzzy pinyin, Shuangpin schemes, and a learning user dictionary.
//!
//! libpinyin's context is process-global; each engine owns an instance.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::PathBuf;
use std::ptr;
use std::sync::{Mutex, OnceLock};

use cosmic_ext_ime_engine::{Candidate, InputEngine, Key, KeyInput, PageInfo, Preedit, Response};
use pinyin_sys as sys;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scheme {
    /// 全拼
    #[default]
    Full,
    /// 双拼 schemes
    DoubleZrm,
    DoubleMs,
    DoubleZiguang,
    DoubleAbc,
    DoublePyjj,
    DoubleXhe,
}

impl Scheme {
    fn is_double(self) -> bool {
        self != Scheme::Full
    }

    fn raw(self) -> sys::DoublePinyinScheme {
        match self {
            Scheme::Full | Scheme::DoubleMs => sys::DOUBLE_PINYIN_MS,
            Scheme::DoubleZrm => sys::DOUBLE_PINYIN_ZRM,
            Scheme::DoubleZiguang => sys::DOUBLE_PINYIN_ZIGUANG,
            Scheme::DoubleAbc => sys::DOUBLE_PINYIN_ABC,
            Scheme::DoublePyjj => sys::DOUBLE_PINYIN_PYJJ,
            Scheme::DoubleXhe => sys::DOUBLE_PINYIN_XHE,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub system_data_dir: PathBuf,
    pub user_data_dir: PathBuf,
    pub scheme: Scheme,
    /// Fuzzy pinyin (z/zh, c/ch, s/sh, n/l, an/ang, en/eng, in/ing, ...).
    pub fuzzy: bool,
    /// Accept incomplete syllables (initials only), e.g. "nh" → 你好.
    pub incomplete: bool,
    pub page_size: usize,
    /// Commit full-width punctuation while idle.
    pub fullwidth_punctuation: bool,
}

impl Default for Config {
    fn default() -> Self {
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."));
        Config {
            system_data_dir: [
                PathBuf::from(sys::PKGDATADIR).join("data"),
                PathBuf::from("/usr/lib/x86_64-linux-gnu/libpinyin/data"),
                PathBuf::from("/usr/lib/aarch64-linux-gnu/libpinyin/data"),
                PathBuf::from("/usr/lib/libpinyin/data"),
                PathBuf::from("/usr/share/libpinyin/data"),
            ]
            .into_iter()
            .find(|d| d.join("merged.bin").exists())
            .unwrap_or_else(|| PathBuf::from(sys::PKGDATADIR).join("data")),
            user_data_dir: data_home.join("cosmic-ext-ime").join("pinyin"),
            scheme: Scheme::Full,
            fuzzy: true,
            incomplete: true,
            page_size: 9,
            fullwidth_punctuation: true,
        }
    }
}

struct Context(*mut sys::pinyin_context_t);
// libpinyin is single-threaded; the context is only touched from the
// daemon's event loop and access is serialised by the mutex below.
unsafe impl Send for Context {}

static CONTEXT: OnceLock<Result<Mutex<Context>, String>> = OnceLock::new();

fn context(cfg: &Config) -> Result<&'static Mutex<Context>, String> {
    CONTEXT
        .get_or_init(|| {
            std::fs::create_dir_all(&cfg.user_data_dir)
                .map_err(|e| format!("creating {}: {e}", cfg.user_data_dir.display()))?;
            if !cfg.system_data_dir.join("merged.bin").exists() {
                return Err(format!(
                    "libpinyin data not found in {} (install libpinyin-data)",
                    cfg.system_data_dir.display()
                ));
            }
            let system = CString::new(cfg.system_data_dir.to_string_lossy().as_bytes()).unwrap();
            let user = CString::new(cfg.user_data_dir.to_string_lossy().as_bytes()).unwrap();
            // SAFETY: valid NUL-terminated paths; libpinyin copies them.
            let ctx = unsafe { sys::pinyin_init(system.as_ptr(), user.as_ptr()) };
            if ctx.is_null() {
                return Err("pinyin_init failed".into());
            }
            log::info!(
                "libpinyin initialised (system {}, user {})",
                cfg.system_data_dir.display(),
                cfg.user_data_dir.display()
            );
            Ok(Mutex::new(Context(ctx)))
        })
        .as_ref()
        .map_err(|e| e.clone())
}

fn options(cfg: &Config) -> sys::pinyin_option_t {
    let mut o =
        sys::IS_PINYIN | sys::USE_DIVIDED_TABLE | sys::USE_RESPLIT_TABLE | sys::DYNAMIC_ADJUST;
    o |= sys::PINYIN_CORRECT_ALL;
    if cfg.incomplete {
        o |= sys::PINYIN_INCOMPLETE;
    }
    if cfg.fuzzy {
        o |= sys::PINYIN_AMB_ALL;
    }
    o
}

fn take_gstring(p: *mut c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: libpinyin returns NUL-terminated strings allocated with g_malloc.
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    unsafe { sys::g_free(p as *mut _) };
    s
}

pub struct PinyinEngine {
    ctx: &'static Mutex<Context>,
    instance: *mut sys::pinyin_instance_t,
    cfg: Config,
    /// Raw keys typed so far (letters and the `'` separator).
    input: String,
    /// Pinyin-key offsets fixed by earlier word selections, innermost last.
    chosen: Vec<usize>,
    candidates: Vec<Candidate>,
    page: usize,
    commits_since_save: u32,
}

impl PinyinEngine {
    pub fn new(cfg: Config) -> Result<Self, String> {
        let ctx = context(&cfg)?;
        let instance = {
            let guard = ctx.lock().unwrap();
            // SAFETY: valid context; options/scheme apply to later parsing.
            unsafe {
                sys::pinyin_set_options(guard.0, options(&cfg));
                sys::pinyin_set_double_pinyin_scheme(guard.0, cfg.scheme.raw());
                sys::pinyin_alloc_instance(guard.0)
            }
        };
        if instance.is_null() {
            return Err("pinyin_alloc_instance failed".into());
        }
        Ok(PinyinEngine {
            ctx,
            instance,
            cfg,
            input: String::new(),
            chosen: Vec::new(),
            candidates: Vec::new(),
            page: 0,
            commits_since_save: 0,
        })
    }

    fn offset(&self) -> usize {
        self.chosen.last().copied().unwrap_or(0)
    }

    /// Number of pinyin keys in the parsed input.
    fn n_pinyin(&self) -> usize {
        let mut offset: usize = 0;
        // SAFETY: valid instance; the cursor is the parsed input length.
        unsafe {
            let len = sys::pinyin_get_parsed_input_length(self.instance);
            sys::pinyin_get_pinyin_offset(self.instance, len, &mut offset);
        }
        offset
    }

    fn parse(&mut self) {
        let text = CString::new(self.input.as_str()).unwrap_or_default();
        // SAFETY: valid instance and NUL-terminated input.
        unsafe {
            if self.cfg.scheme.is_double() {
                sys::pinyin_parse_more_double_pinyins(self.instance, text.as_ptr());
            } else {
                sys::pinyin_parse_more_full_pinyins(self.instance, text.as_ptr());
            }
            sys::pinyin_guess_sentence(self.instance);
        }
    }

    fn update_candidates(&mut self) {
        self.candidates.clear();
        self.page = 0;
        if self.input.is_empty() {
            return;
        }
        let offset = self.offset();
        // SAFETY: valid instance; candidate pointers stay valid until the next guess.
        unsafe {
            if !sys::pinyin_guess_candidates(
                self.instance,
                offset,
                sys::SORT_BY_PHRASE_LENGTH_AND_PINYIN_LENGTH_AND_FREQUENCY,
            ) {
                return;
            }
            let mut n: u32 = 0;
            sys::pinyin_get_n_candidate(self.instance, &mut n);
            for i in 0..n {
                let mut cand: *mut sys::lookup_candidate_t = ptr::null_mut();
                if !sys::pinyin_get_candidate(self.instance, i, &mut cand) || cand.is_null() {
                    continue;
                }
                let mut s: *const c_char = ptr::null();
                if !sys::pinyin_get_candidate_string(self.instance, cand, &mut s) || s.is_null() {
                    continue;
                }
                let text = CStr::from_ptr(s).to_string_lossy().into_owned();
                if !text.is_empty() {
                    self.candidates.push(Candidate { text, hint: None });
                }
            }
        }
    }

    /// The best guess for the whole input, honouring earlier selections.
    fn sentence(&self) -> String {
        let mut p: *mut c_char = ptr::null_mut();
        // SAFETY: valid instance; the string is g_malloc'd for us.
        unsafe { sys::pinyin_get_sentence(self.instance, 0, &mut p) };
        take_gstring(p)
    }

    /// Segmented pinyin for the popup header, e.g. "ni'hao|".
    fn auxiliary(&self) -> String {
        let mut p: *mut c_char = ptr::null_mut();
        let cursor = self.input.len();
        // SAFETY: valid instance; cursor within the parsed input.
        unsafe {
            if self.cfg.scheme.is_double() {
                sys::pinyin_get_double_pinyin_auxiliary_text(self.instance, cursor, &mut p);
            } else {
                sys::pinyin_get_full_pinyin_auxiliary_text(self.instance, cursor, &mut p);
            }
        }
        take_gstring(p)
    }

    fn page_slice(&self) -> &[Candidate] {
        let start = (self.page * self.cfg.page_size).min(self.candidates.len());
        let end = (start + self.cfg.page_size).min(self.candidates.len());
        &self.candidates[start..end]
    }

    fn total_pages(&self) -> usize {
        self.candidates.len().div_ceil(self.cfg.page_size).max(1)
    }

    fn select(&mut self, index_in_page: usize) -> Response {
        let index = self.page * self.cfg.page_size + index_in_page;
        if index >= self.candidates.len() {
            return Response::Consumed;
        }
        let offset = self.offset();
        let mut cand: *mut sys::lookup_candidate_t = ptr::null_mut();
        // Re-run the guess so libpinyin's candidate pointers match our list.
        // SAFETY: valid instance; index < count from the same guess.
        let new_offset = unsafe {
            sys::pinyin_guess_candidates(
                self.instance,
                offset,
                sys::SORT_BY_PHRASE_LENGTH_AND_PINYIN_LENGTH_AND_FREQUENCY,
            );
            if !sys::pinyin_get_candidate(self.instance, index as u32, &mut cand) || cand.is_null()
            {
                return Response::Consumed;
            }
            sys::pinyin_choose_candidate(self.instance, offset, cand)
        };
        if new_offset < 0 {
            return Response::Consumed;
        }
        let new_offset = new_offset as usize;
        if new_offset >= self.n_pinyin() {
            return self.commit_sentence();
        }
        self.chosen.push(new_offset);
        // SAFETY: valid instance.
        unsafe { sys::pinyin_guess_sentence(self.instance) };
        self.update_candidates();
        Response::Consumed
    }

    fn commit_sentence(&mut self) -> Response {
        let text = self.sentence();
        // SAFETY: valid instance; index 0 is the sentence we just read.
        unsafe { sys::pinyin_train(self.instance, 0) };
        self.commits_since_save += 1;
        if self.commits_since_save >= 10 {
            self.save();
        }
        self.reset();
        Response::Commit(text)
    }

    fn save(&mut self) {
        self.commits_since_save = 0;
        let guard = self.ctx.lock().unwrap();
        // SAFETY: valid context.
        unsafe { sys::pinyin_save(guard.0) };
    }

    fn push_key(&mut self, c: char) -> Response {
        self.input.push(c);
        self.parse();
        self.update_candidates();
        Response::Consumed
    }

    fn backspace(&mut self) -> Response {
        if self.input.is_empty() {
            return Response::Ignored;
        }
        if let Some(offset) = self.chosen.pop() {
            // Undo the last word selection first, like ibus-libpinyin.
            // SAFETY: valid instance; offset came from choose_candidate.
            unsafe {
                sys::pinyin_clear_constraint(self.instance, self.offset());
                let _ = offset;
                sys::pinyin_guess_sentence(self.instance);
            }
        } else {
            self.input.pop();
            self.parse();
        }
        self.update_candidates();
        Response::Consumed
    }

    fn punctuation(c: char) -> Option<&'static str> {
        Some(match c {
            ',' => "，",
            '.' => "。",
            '?' => "？",
            '!' => "！",
            ':' => "：",
            ';' => "；",
            '(' => "（",
            ')' => "）",
            '<' => "《",
            '>' => "》",
            '\\' => "、",
            '~' => "～",
            '^' => "……",
            '_' => "——",
            '$' => "￥",
            _ => return None,
        })
    }
}

impl Drop for PinyinEngine {
    fn drop(&mut self) {
        if self.commits_since_save > 0 {
            self.save();
        }
        // SAFETY: instance was allocated by us and is freed once.
        unsafe { sys::pinyin_free_instance(self.instance) };
    }
}

impl InputEngine for PinyinEngine {
    fn name(&self) -> &str {
        if self.cfg.scheme.is_double() {
            "双拼"
        } else {
            "拼音"
        }
    }

    fn process_key(&mut self, input: KeyInput) -> Response {
        let m = input.modifiers;
        if m.ctrl || m.alt || m.logo {
            return Response::Ignored;
        }
        let composing = !self.input.is_empty();
        match input.key {
            Key::Char(c) if c.is_ascii_lowercase() => self.push_key(c),
            Key::Char('\'') if composing => self.push_key('\''),
            Key::Char(d) if composing && d.is_ascii_digit() && d != '0' => {
                self.select(d as usize - '1' as usize)
            }
            Key::Space if composing => self.select(0),
            Key::Enter if composing => {
                let raw = std::mem::take(&mut self.input);
                self.reset();
                Response::Commit(raw)
            }
            Key::Backspace => self.backspace(),
            Key::Escape if composing => {
                self.reset();
                Response::Consumed
            }
            Key::PageDown | Key::Down if composing => {
                self.page = (self.page + 1) % self.total_pages();
                Response::Consumed
            }
            Key::PageUp | Key::Up if composing => {
                self.page = (self.page + self.total_pages() - 1) % self.total_pages();
                Response::Consumed
            }
            Key::Char(c) if !composing && self.cfg.fullwidth_punctuation => {
                match Self::punctuation(c) {
                    Some(p) => Response::Commit(p.to_string()),
                    None => Response::Ignored,
                }
            }
            Key::Char(c) if composing => {
                // Punctuation ends the sentence: commit the best guess, then the mark.
                let sentence = match self.commit_sentence() {
                    Response::Commit(t) => t,
                    _ => String::new(),
                };
                match Self::punctuation(c).filter(|_| self.cfg.fullwidth_punctuation) {
                    Some(p) => Response::Commit(sentence + p),
                    None => Response::CommitAndForward(sentence),
                }
            }
            _ if composing => Response::Consumed,
            _ => Response::Ignored,
        }
    }

    fn preedit(&self) -> Preedit {
        if self.input.is_empty() {
            return Preedit::default();
        }
        let text = self.sentence();
        let cursor = text.len();
        Preedit { text, cursor }
    }

    fn candidates(&self) -> Vec<Candidate> {
        self.page_slice().to_vec()
    }

    fn page_info(&self) -> PageInfo {
        PageInfo {
            page: self.page,
            total_pages: Some(self.total_pages()),
            has_next: self.page + 1 < self.total_pages(),
            page_size: self.cfg.page_size,
        }
    }

    fn selected(&self) -> usize {
        0
    }

    fn reset(&mut self) {
        self.input.clear();
        self.chosen.clear();
        self.candidates.clear();
        self.page = 0;
        // SAFETY: valid instance.
        unsafe { sys::pinyin_reset(self.instance) };
    }

    fn is_composing(&self) -> bool {
        !self.input.is_empty()
    }

    fn header(&self) -> String {
        if self.input.is_empty() {
            String::new()
        } else {
            self.auxiliary()
        }
    }
}
