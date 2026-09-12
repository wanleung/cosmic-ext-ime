//! Turns the shared [`PopeinputConfig`] into what the engine and popup need.

use anyhow::{Context, Result};
use popeinput_cangjie::{CangjieEngine, Config as EngineConfig, Filter, Mode, Version};
use popeinput_config::{CangjieVersion, CharSet, PopeinputConfig};
use popeinput_engine::InputEngine;

pub fn build_engine(cfg: &PopeinputConfig) -> Result<Box<dyn InputEngine>> {
    let mode = match cfg.mode {
        popeinput_config::Mode::Cangjie => Mode::Cangjie,
        popeinput_config::Mode::Quick => Mode::Quick,
    };
    let version = match cfg.cangjie_version {
        CangjieVersion::V3 => Version::V3,
        CangjieVersion::V5 => Version::V5,
    };
    let filter = cfg
        .char_sets
        .iter()
        .map(|s| match s {
            CharSet::Big5 => Filter::BIG5,
            CharSet::Hkscs => Filter::HKSCS,
            CharSet::Chinese => Filter::CHINESE,
            CharSet::Kanji => Filter::KANJI,
            CharSet::Hiragana => Filter::HIRAGANA,
            CharSet::Katakana => Filter::KATAKANA,
            CharSet::Zhuyin => Filter::ZHUYIN,
            CharSet::Punctuation => Filter::PUNCTUATION,
            CharSet::Symbols => Filter::SYMBOLS,
        })
        .reduce(|a, b| a | b)
        .unwrap_or(Filter::HONG_KONG);
    let engine = CangjieEngine::new(EngineConfig {
        mode,
        version,
        filter,
        page_size: (cfg.page_size as usize).clamp(1, 9),
        fullwidth_chars: cfg.fullwidth_chars,
    })
    .context("failed to open libcangjie2 database")?;
    Ok(Box::new(engine))
}
