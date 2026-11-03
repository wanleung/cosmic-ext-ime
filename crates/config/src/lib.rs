//! The settings contract between the daemon and any settings UI.
//!
//! Stored through cosmic-config under `~/.config/cosmic/<APP_ID>/v1/<field>`,
//! one RON file per field, exactly like COSMIC's own components. Any writer
//! (our settings app, a future COSMIC Settings page, a text editor) can change
//! a key and the daemon picks it up live.

use cosmic_config::cosmic_config_derive::CosmicConfigEntry;
use cosmic_config::{Config, ConfigGet, CosmicConfigEntry};
use serde::{Deserialize, Serialize};

pub const APP_ID: &str = "io.github.wanleung.CosmicExtIme";
pub const CONFIG_VERSION: u64 = 1;
pub const STATE_VERSION: u64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Engine {
    /// libcangjie2: Cangjie / Quick, tuned for Hong Kong users.
    #[default]
    Cangjie,
    /// librime: any RIME schema (Cangjie, Quick, Jyutping, Pinyin, ...).
    Rime,
    /// libpinyin: smart Pinyin / Shuangpin with sentence prediction.
    Pinyin,
    /// libchewing: Traditional Chinese Zhuyin (新酷音).
    Chewing,
}

impl Engine {
    pub const ALL: [Engine; 4] = [
        Engine::Cangjie,
        Engine::Rime,
        Engine::Pinyin,
        Engine::Chewing,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Engine::Cangjie => "倉頡／速成 (libcangjie)",
            Engine::Rime => "RIME 中州韻",
            Engine::Pinyin => "智能拼音 (libpinyin)",
            Engine::Chewing => "新酷音 (libchewing)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PinyinScheme {
    #[default]
    Full,
    DoubleZrm,
    DoubleMs,
    DoubleZiguang,
    DoubleAbc,
    DoublePyjj,
    DoubleXhe,
}

impl PinyinScheme {
    pub const ALL: [PinyinScheme; 7] = [
        PinyinScheme::Full,
        PinyinScheme::DoubleZrm,
        PinyinScheme::DoubleMs,
        PinyinScheme::DoubleZiguang,
        PinyinScheme::DoubleAbc,
        PinyinScheme::DoublePyjj,
        PinyinScheme::DoubleXhe,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PinyinScheme::Full => "全拼",
            PinyinScheme::DoubleZrm => "双拼 · 自然码",
            PinyinScheme::DoubleMs => "双拼 · 微软",
            PinyinScheme::DoubleZiguang => "双拼 · 紫光",
            PinyinScheme::DoubleAbc => "双拼 · 智能ABC",
            PinyinScheme::DoublePyjj => "双拼 · 拼音加加",
            PinyinScheme::DoubleXhe => "双拼 · 小鹤",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Mode {
    #[default]
    Cangjie,
    Quick,
}

impl Mode {
    pub const ALL: [Mode; 2] = [Mode::Cangjie, Mode::Quick];

    pub fn label(self) -> &'static str {
        match self {
            Mode::Cangjie => "倉頡 Cangjie",
            Mode::Quick => "速成 Quick",
        }
    }
}

/// Keyboard layouts understood by libchewing's `chewing_KBStr2Num()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ChewingKeyboard {
    /// Standard Zhuyin (大千式).
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

impl ChewingKeyboard {
    pub const ALL: [ChewingKeyboard; 9] = [
        ChewingKeyboard::Default,
        ChewingKeyboard::Hsu,
        ChewingKeyboard::Ibm,
        ChewingKeyboard::GinYieh,
        ChewingKeyboard::Et,
        ChewingKeyboard::Et26,
        ChewingKeyboard::Dvorak,
        ChewingKeyboard::DvorakHsu,
        ChewingKeyboard::DachenCp26,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ChewingKeyboard::Default => "標準注音 (大千式)",
            ChewingKeyboard::Hsu => "許氏注音 (Hsu)",
            ChewingKeyboard::Ibm => "IBM 注音",
            ChewingKeyboard::GinYieh => "精業注音 (Gin-Yieh)",
            ChewingKeyboard::Et => "倚天注音 (ET)",
            ChewingKeyboard::Et26 => "倚天 26 鍵 (ET26)",
            ChewingKeyboard::Dvorak => "Dvorak 注音",
            ChewingKeyboard::DvorakHsu => "Dvorak 許氏注音",
            ChewingKeyboard::DachenCp26 => "大千 CP26",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CangjieVersion {
    V3,
    #[default]
    V5,
}

impl CangjieVersion {
    pub const ALL: [CangjieVersion; 2] = [CangjieVersion::V3, CangjieVersion::V5];

    pub fn label(self) -> &'static str {
        match self {
            CangjieVersion::V3 => "倉頡三代 (Cangjie 3)",
            CangjieVersion::V5 => "倉頡五代 (Cangjie 5)",
        }
    }
}

/// Character sets in libcangjie's database; maps 1:1 to `CANGJIE_FILTER_*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CharSet {
    Big5,
    Hkscs,
    Chinese,
    Kanji,
    Hiragana,
    Katakana,
    Zhuyin,
    Punctuation,
    Symbols,
}

impl CharSet {
    pub const ALL: [CharSet; 9] = [
        CharSet::Big5,
        CharSet::Hkscs,
        CharSet::Chinese,
        CharSet::Kanji,
        CharSet::Hiragana,
        CharSet::Katakana,
        CharSet::Zhuyin,
        CharSet::Punctuation,
        CharSet::Symbols,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CharSet::Big5 => "Big5 常用字",
            CharSet::Hkscs => "香港增補字符集 HKSCS",
            CharSet::Chinese => "所有中文字 (all Chinese)",
            CharSet::Kanji => "日文漢字 Kanji",
            CharSet::Hiragana => "平假名 Hiragana",
            CharSet::Katakana => "片假名 Katakana",
            CharSet::Zhuyin => "注音符號 Zhuyin",
            CharSet::Punctuation => "標點符號 Punctuation",
            CharSet::Symbols => "符號 Symbols",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            CharSet::Big5 => "Traditional Chinese, the everyday set",
            CharSet::Hkscs => "Hong Kong characters not in Big5 (e.g. 嘅 啲)",
            CharSet::Chinese => {
                "Every Chinese character in the table, including rare and simplified forms"
            }
            CharSet::Kanji => "All CJK Unified Ideographs used in Japanese",
            CharSet::Hiragana => "Type with zj + romaji, e.g. zja → あ",
            CharSet::Katakana => "Type with zj + romaji, e.g. zja → ア",
            CharSet::Zhuyin => "ㄅㄆㄇㄈ",
            CharSet::Punctuation => "Full-width punctuation with z-codes, e.g. zxab → ，",
            CharSet::Symbols => "Arrows, box drawing, units and other symbols",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, CosmicConfigEntry)]
#[version = 1]
pub struct PopeinputConfig {
    pub engine: Engine,
    /// RIME schema id (e.g. "cangjie5"); empty means RIME's own default.
    pub rime_schema: String,
    pub pinyin_scheme: PinyinScheme,
    /// Fuzzy pinyin: z/zh, c/ch, s/sh, n/l, an/ang, en/eng, in/ing, ...
    pub pinyin_fuzzy: bool,
    /// Accept incomplete syllables (initials), e.g. "nh" for 你好.
    pub pinyin_incomplete: bool,
    /// libchewing keyboard layout.
    pub chewing_keyboard: ChewingKeyboard,
    /// Let Space select the highlighted candidate instead of paging.
    pub chewing_space_as_selection: bool,
    /// Learn new phrases into libchewing's user dictionary.
    pub chewing_auto_learn: bool,
    /// Candidates per page for Chewing (libchewing supports 1–10).
    pub chewing_page_size: u32,
    /// Selection keys, in page order.  Default is "1234567890".
    pub chewing_sel_keys: String,
    /// ibus-chewing "easy symbol input" (Shift+letter symbols).
    pub chewing_easy_symbol_input: bool,
    /// Move the cursor after selecting a candidate.
    pub chewing_auto_shift_cur: bool,
    /// Add learned phrases in the forward direction.
    pub chewing_add_phrase_direction: bool,
    /// Prefer phrases behind the cursor.
    pub chewing_phrase_choice_from_last: bool,
    /// Escape clears the whole preedit buffer.
    pub chewing_esc_clean_all_buf: bool,
    /// Plain Zhuyin: pick every character yourself, no phrase conversion.
    pub chewing_plain_zhuyin: bool,
    /// Maximum number of Chinese symbols in one composition.
    pub chewing_max_chi_symbol_len: u32,
    pub mode: Mode,
    pub cangjie_version: CangjieVersion,
    pub char_sets: Vec<CharSet>,
    /// Candidates per page, 1–9 (selected with the number keys).
    pub page_size: u32,
    /// Commit full-width digits, punctuation and space while idle.
    pub fullwidth_chars: bool,
    /// Chinese input is on only while a matching keyboard layout is active,
    /// so COSMIC's input-source switcher and panel applet control it.
    pub follow_layout: bool,
    /// Case-insensitive substrings matched against the active layout's name.
    pub chinese_layouts: Vec<String>,
    pub ctrl_space_toggle: bool,
    pub shift_tap_toggle: bool,
    pub popup_font_size: u32,
    /// Lay the candidates out in one row (ibus-chewing's horizontal lookup
    /// table) instead of one per line.
    pub popup_horizontal: bool,
}

impl Default for PopeinputConfig {
    fn default() -> Self {
        PopeinputConfig {
            engine: Engine::Cangjie,
            rime_schema: String::new(),
            pinyin_scheme: PinyinScheme::Full,
            pinyin_fuzzy: true,
            pinyin_incomplete: true,
            chewing_keyboard: ChewingKeyboard::Default,
            chewing_space_as_selection: true,
            chewing_auto_learn: true,
            chewing_page_size: 10,
            chewing_sel_keys: "1234567890".into(),
            chewing_easy_symbol_input: true,
            chewing_auto_shift_cur: true,
            chewing_add_phrase_direction: true,
            chewing_phrase_choice_from_last: true,
            chewing_esc_clean_all_buf: false,
            chewing_plain_zhuyin: false,
            chewing_max_chi_symbol_len: 20,
            mode: Mode::Cangjie,
            cangjie_version: CangjieVersion::V5,
            char_sets: vec![CharSet::Big5, CharSet::Hkscs],
            page_size: 9,
            // ibus-chewing starts in half shape; full-width punctuation and
            // easy symbols do not depend on this (see the Chewing engine).
            fullwidth_chars: false,
            follow_layout: true,
            chinese_layouts: vec![
                "chinese".into(),
                "cantonese".into(),
                "hong kong".into(),
                "taiwanese".into(),
            ],
            ctrl_space_toggle: false,
            shift_tap_toggle: false,
            popup_font_size: 18,
            popup_horizontal: false,
        }
    }
}

impl PopeinputConfig {
    pub fn handler() -> Result<Config, cosmic_config::Error> {
        Config::new(APP_ID, CONFIG_VERSION)
    }

    /// Load from disk; missing keys fall back to defaults and are written out
    /// so that every key exists for other writers to find.
    pub fn load(config: &Config) -> Self {
        let (cfg, complete) = match Self::get_entry(config) {
            Ok(cfg) => (cfg, true),
            Err((_errors, cfg)) => (cfg, false),
        };
        let store_empty = ConfigGet::get::<Mode>(config, "mode").is_err();
        if !complete || store_empty {
            if let Err(e) = cfg.write_entry(config) {
                eprintln!("cosmic-ext-ime: could not write default settings: {e}");
            }
        }
        cfg
    }

    pub fn is_chinese_layout(&self, layout_name: &str) -> bool {
        let name = layout_name.to_ascii_lowercase();
        self.chinese_layouts
            .iter()
            .any(|p| name.contains(&p.to_ascii_lowercase()))
    }

    pub fn has_char_set(&self, set: CharSet) -> bool {
        self.char_sets.contains(&set)
    }

    /// The settings a change to which requires rebuilding the engine.
    pub fn engine_fields(&self) -> impl PartialEq {
        (
            self.engine,
            self.rime_schema.clone(),
            self.pinyin_scheme,
            self.pinyin_fuzzy,
            self.pinyin_incomplete,
            (
                self.chewing_keyboard,
                self.chewing_space_as_selection,
                self.chewing_auto_learn,
                self.chewing_page_size,
                self.chewing_sel_keys.clone(),
                self.chewing_easy_symbol_input,
                self.chewing_auto_shift_cur,
                self.chewing_add_phrase_direction,
                self.chewing_phrase_choice_from_last,
                self.chewing_esc_clean_all_buf,
                self.chewing_plain_zhuyin,
                self.chewing_max_chi_symbol_len,
            ),
            self.mode,
            self.cangjie_version,
            self.char_sets.clone(),
            self.page_size,
            self.fullwidth_chars,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RimeSchema {
    pub id: String,
    pub name: String,
}

/// Runtime facts the daemon publishes for the settings UI (cosmic-config
/// *state*, under ~/.local/state), so the UI never needs to link librime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, CosmicConfigEntry, Default)]
#[version = 1]
pub struct RimeState {
    /// Schemas RIME has deployed; filled once the RIME engine has started.
    pub schemas: Vec<RimeSchema>,
    /// Why RIME could not start, or empty.
    pub error: String,
    /// Bumped by the settings app to ask the daemon to redeploy RIME
    /// (needed after editing *.custom.yaml); the daemon records the value
    /// it last served in `deploy_done`.
    pub deploy_requested: u64,
    pub deploy_done: u64,
}

impl RimeState {
    pub fn handler() -> Result<Config, cosmic_config::Error> {
        Config::new_state(APP_ID, STATE_VERSION)
    }

    pub fn load(config: &Config) -> Self {
        Self::get_entry(config).unwrap_or_else(|(_, s)| s)
    }
}

/// Where RIME keeps user schemas, custom.yaml patches and dictionaries.
pub fn rime_user_data_dir() -> std::path::PathBuf {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share"))
        })
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    data_home.join("cosmic-ext-ime").join("rime")
}
