//! Turns the shared [`PopeinputConfig`] into what the engine and popup need.

use anyhow::{Context, Result};
use cosmic_config::CosmicConfigEntry;
use popeinput_cangjie::{CangjieEngine, Config as EngineConfig, Filter, Mode, Version};
use popeinput_config::{CangjieVersion, CharSet, Engine, PopeinputConfig, RimeSchema, RimeState};
use popeinput_engine::InputEngine;

pub fn build_engine(cfg: &PopeinputConfig) -> Result<Box<dyn InputEngine>> {
    match cfg.engine {
        Engine::Cangjie => build_cangjie(cfg),
        Engine::Rime => {
            let result = build_rime(cfg);
            publish_rime_state(result.as_ref().err());
            result
        }
    }
}

fn build_rime(cfg: &PopeinputConfig) -> Result<Box<dyn InputEngine>> {
    let rime_cfg = popeinput_rime::Config {
        schema: Some(cfg.rime_schema.clone()).filter(|s| !s.is_empty()),
        page_size: cfg.page_size as usize,
        ..popeinput_rime::Config::default()
    };
    let engine = popeinput_rime::RimeEngine::new(rime_cfg)
        .map_err(|e| anyhow::anyhow!(e))
        .context("failed to start RIME")?;
    Ok(Box::new(engine))
}

fn publish_rime_state(error: Option<&anyhow::Error>) {
    let state = RimeState {
        schemas: popeinput_rime::schemas()
            .into_iter()
            .map(|s| RimeSchema {
                id: s.id,
                name: s.name,
            })
            .collect(),
        error: error.map(|e| format!("{e:#}")).unwrap_or_default(),
    };
    match RimeState::handler() {
        Ok(h) => {
            if let Err(e) = state.write_entry(&h) {
                log::warn!("could not publish RIME state: {e}");
            }
        }
        Err(e) => log::warn!("no state store for RIME schemas: {e}"),
    }
}

fn build_cangjie(cfg: &PopeinputConfig) -> Result<Box<dyn InputEngine>> {
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
