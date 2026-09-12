use std::path::PathBuf;

use anyhow::{Context, Result};
use popeinput_cangjie::{Filter, Mode, Version};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// "cangjie" or "quick"
    pub mode: String,
    /// Cangjie version: 3 or 5
    pub version: u8,
    /// Character sets: big5, hkscs, punctuation, chinese, zhuyin, kanji,
    /// katakana, hiragana, symbols
    pub filters: Vec<String>,
    pub page_size: usize,
    /// Commit full-width digits, punctuation and space while idle.
    pub fullwidth_chars: bool,
    pub toggle: Toggle,
    pub popup: Popup,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Toggle {
    /// Turn Chinese input on only while a matching keyboard layout is active,
    /// so COSMIC's Input Sources switcher (Super+Space) and panel indicator
    /// control it. Ignored when only one layout is configured.
    pub follow_layout: bool,
    /// Case-insensitive substrings matched against the active layout's name.
    pub chinese_layouts: Vec<String>,
    pub ctrl_space: bool,
    pub shift_tap: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Popup {
    pub font_size: f32,
    /// Hex RGB colours.
    pub background: String,
    pub foreground: String,
    pub hint: String,
    pub border: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            mode: "cangjie".into(),
            version: 5,
            filters: vec!["big5".into(), "hkscs".into()],
            page_size: 9,
            fullwidth_chars: true,
            toggle: Toggle::default(),
            popup: Popup::default(),
        }
    }
}

impl Default for Toggle {
    fn default() -> Self {
        Toggle {
            follow_layout: true,
            chinese_layouts: vec![
                "chinese".into(),
                "cantonese".into(),
                "hong kong".into(),
                "taiwanese".into(),
            ],
            ctrl_space: false,
            shift_tap: false,
        }
    }
}

impl Default for Popup {
    fn default() -> Self {
        Popup {
            font_size: 18.0,
            background: "#1e1e1e".into(),
            foreground: "#f0f0f0".into(),
            hint: "#8a8a8a".into(),
            border: "#5a5a5a".into(),
        }
    }
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("popeinput").join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let Some(path) = Self::path() else { return Ok(Self::default()) };
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn mode(&self) -> Result<Mode> {
        match self.mode.to_ascii_lowercase().as_str() {
            "cangjie" | "倉頡" => Ok(Mode::Cangjie),
            "quick" | "速成" => Ok(Mode::Quick),
            other => anyhow::bail!("unknown mode {other:?} (expected \"cangjie\" or \"quick\")"),
        }
    }

    pub fn version(&self) -> Result<Version> {
        match self.version {
            3 => Ok(Version::V3),
            5 => Ok(Version::V5),
            other => anyhow::bail!("unknown Cangjie version {other} (expected 3 or 5)"),
        }
    }

    pub fn filter(&self) -> Result<Filter> {
        let mut filter = None;
        for name in &self.filters {
            let f = match name.to_ascii_lowercase().as_str() {
                "big5" => Filter::BIG5,
                "hkscs" => Filter::HKSCS,
                "punctuation" => Filter::PUNCTUATION,
                "chinese" => Filter::CHINESE,
                "zhuyin" => Filter::ZHUYIN,
                "kanji" => Filter::KANJI,
                "katakana" => Filter::KATAKANA,
                "hiragana" => Filter::HIRAGANA,
                "symbols" => Filter::SYMBOLS,
                other => anyhow::bail!("unknown filter {other:?}"),
            };
            filter = Some(filter.map_or(f, |acc: Filter| acc | f));
        }
        Ok(filter.unwrap_or(Filter::HONG_KONG))
    }

    pub fn is_chinese_layout(&self, layout_name: &str) -> bool {
        let name = layout_name.to_ascii_lowercase();
        self.toggle
            .chinese_layouts
            .iter()
            .any(|p| name.contains(&p.to_ascii_lowercase()))
    }
}

pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}
