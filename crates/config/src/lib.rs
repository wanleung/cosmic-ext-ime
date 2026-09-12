//! The settings contract between the daemon and any settings UI.
//!
//! Stored through cosmic-config under `~/.config/cosmic/<APP_ID>/v1/<field>`,
//! one RON file per field, exactly like COSMIC's own components. Any writer
//! (our settings app, a future COSMIC Settings page, a text editor) can change
//! a key and the daemon picks it up live.

use cosmic_config::cosmic_config_derive::CosmicConfigEntry;
use cosmic_config::{Config, ConfigGet, CosmicConfigEntry};
use serde::{Deserialize, Serialize};

pub const APP_ID: &str = "io.github.wanleung.popeinput";
pub const CONFIG_VERSION: u64 = 1;
pub const STATE_VERSION: u64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Engine {
    /// libcangjie2: Cangjie / Quick, tuned for Hong Kong users.
    #[default]
    Cangjie,
    /// librime: any RIME schema (Cangjie, Quick, Jyutping, Pinyin, ...).
    Rime,
}

impl Engine {
    pub const ALL: [Engine; 2] = [Engine::Cangjie, Engine::Rime];

    pub fn label(self) -> &'static str {
        match self {
            Engine::Cangjie => "倉頡／速成 (libcangjie)",
            Engine::Rime => "RIME 中州韻",
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
            CharSet::Chinese => "Every Chinese character in the table, including rare and simplified forms",
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
}

impl Default for PopeinputConfig {
    fn default() -> Self {
        PopeinputConfig {
            engine: Engine::Cangjie,
            rime_schema: String::new(),
            mode: Mode::Cangjie,
            cangjie_version: CangjieVersion::V5,
            char_sets: vec![CharSet::Big5, CharSet::Hkscs],
            page_size: 9,
            fullwidth_chars: true,
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
                eprintln!("popeinput: could not write default settings: {e}");
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
}

impl RimeState {
    pub fn handler() -> Result<Config, cosmic_config::Error> {
        Config::new_state(APP_ID, STATE_VERSION)
    }

    pub fn load(config: &Config) -> Self {
        Self::get_entry(config).unwrap_or_else(|(_, s)| s)
    }
}
