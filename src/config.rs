//! Turns the shared [`PopeinputConfig`] into what the engine and popup need.

use anyhow::{Context, Result};
use cosmic_config::CosmicConfigEntry;
use cosmic_ext_ime_cangjie::{CangjieEngine, Config as EngineConfig, Filter, Mode, Version};
use cosmic_ext_ime_chewing::{
    ChewingEngine, Config as ChewingConfig, Keyboard as ChewingKeyboardLayout,
};
use cosmic_ext_ime_config::{
    CangjieVersion, CharSet, ChewingKeyboard, Engine, PinyinScheme, PopeinputConfig, RimeSchema,
    RimeState,
};
use cosmic_ext_ime_engine::InputEngine;

pub fn build_engine(cfg: &PopeinputConfig) -> Result<Box<dyn InputEngine>> {
    match cfg.engine {
        Engine::Cangjie => build_cangjie(cfg),
        Engine::Rime => {
            let result = build_rime(cfg);
            publish_rime_state(result.as_ref().err());
            result
        }
        Engine::Pinyin => build_pinyin(cfg),
        Engine::Chewing => build_chewing(cfg),
    }
}

fn build_pinyin(cfg: &PopeinputConfig) -> Result<Box<dyn InputEngine>> {
    use cosmic_ext_ime_pinyin::Scheme;
    let scheme = match cfg.pinyin_scheme {
        PinyinScheme::Full => Scheme::Full,
        PinyinScheme::DoubleZrm => Scheme::DoubleZrm,
        PinyinScheme::DoubleMs => Scheme::DoubleMs,
        PinyinScheme::DoubleZiguang => Scheme::DoubleZiguang,
        PinyinScheme::DoubleAbc => Scheme::DoubleAbc,
        PinyinScheme::DoublePyjj => Scheme::DoublePyjj,
        PinyinScheme::DoubleXhe => Scheme::DoubleXhe,
    };
    let engine = cosmic_ext_ime_pinyin::PinyinEngine::new(cosmic_ext_ime_pinyin::Config {
        scheme,
        fuzzy: cfg.pinyin_fuzzy,
        incomplete: cfg.pinyin_incomplete,
        page_size: (cfg.page_size as usize).clamp(1, 9),
        fullwidth_punctuation: cfg.fullwidth_chars,
        ..cosmic_ext_ime_pinyin::Config::default()
    })
    .map_err(|e| anyhow::anyhow!(e))
    .context("failed to start libpinyin")?;
    Ok(Box::new(engine))
}

fn build_chewing(cfg: &PopeinputConfig) -> Result<Box<dyn InputEngine>> {
    let keyboard = match cfg.chewing_keyboard {
        ChewingKeyboard::Default => ChewingKeyboardLayout::Default,
        ChewingKeyboard::Hsu => ChewingKeyboardLayout::Hsu,
        ChewingKeyboard::Ibm => ChewingKeyboardLayout::Ibm,
        ChewingKeyboard::GinYieh => ChewingKeyboardLayout::GinYieh,
        ChewingKeyboard::Et => ChewingKeyboardLayout::Et,
        ChewingKeyboard::Et26 => ChewingKeyboardLayout::Et26,
        ChewingKeyboard::Dvorak => ChewingKeyboardLayout::Dvorak,
        ChewingKeyboard::DvorakHsu => ChewingKeyboardLayout::DvorakHsu,
        ChewingKeyboard::DachenCp26 => ChewingKeyboardLayout::DachenCp26,
    };
    let engine = ChewingEngine::new(ChewingConfig {
        keyboard,
        page_size: (cfg.chewing_page_size as usize).clamp(1, 10),
        space_as_selection: cfg.chewing_space_as_selection,
        auto_learn: cfg.chewing_auto_learn,
        fullwidth_chars: cfg.fullwidth_chars,
        selection_keys: cfg.chewing_sel_keys.clone(),
        easy_symbol_input: cfg.chewing_easy_symbol_input,
        auto_shift_cur: cfg.chewing_auto_shift_cur,
        add_phrase_direction: cfg.chewing_add_phrase_direction,
        phrase_choice_from_last: cfg.chewing_phrase_choice_from_last,
        esc_clean_all_buf: cfg.chewing_esc_clean_all_buf,
        plain_zhuyin: cfg.chewing_plain_zhuyin,
        max_chi_symbol_len: cfg.chewing_max_chi_symbol_len as usize,
        ..ChewingConfig::default()
    })
    .map_err(|e| anyhow::anyhow!(e))
    .context("failed to start libchewing")?;
    Ok(Box::new(engine))
}

fn build_rime(cfg: &PopeinputConfig) -> Result<Box<dyn InputEngine>> {
    let rime_cfg = cosmic_ext_ime_rime::Config {
        schema: Some(cfg.rime_schema.clone()).filter(|s| !s.is_empty()),
        page_size: cfg.page_size as usize,
        ..cosmic_ext_ime_rime::Config::default()
    };
    let engine = cosmic_ext_ime_rime::RimeEngine::new(rime_cfg)
        .map_err(|e| anyhow::anyhow!(e))
        .context("failed to start RIME")?;
    Ok(Box::new(engine))
}

fn publish_rime_state(error: Option<&anyhow::Error>) {
    let Ok(handler) = RimeState::handler() else {
        log::warn!("no state store for RIME schemas");
        return;
    };
    let previous = RimeState::load(&handler);
    let state = RimeState {
        schemas: cosmic_ext_ime_rime::schemas()
            .into_iter()
            .map(|s| RimeSchema {
                id: s.id,
                name: s.name,
            })
            .collect(),
        error: error.map(|e| format!("{e:#}")).unwrap_or_default(),
        deploy_requested: previous.deploy_requested,
        deploy_done: previous.deploy_requested,
    };
    if let Err(e) = state.write_entry(&handler) {
        log::warn!("could not publish RIME state: {e}");
    }
}

/// True when the settings app asked for a RIME redeploy we have not served.
pub fn rime_deploy_pending(state: &RimeState) -> bool {
    state.deploy_requested > state.deploy_done
}

pub fn redeploy_rime() -> Result<()> {
    cosmic_ext_ime_rime::redeploy().map_err(|e| anyhow::anyhow!(e))
}

fn build_cangjie(cfg: &PopeinputConfig) -> Result<Box<dyn InputEngine>> {
    let mode = match cfg.mode {
        cosmic_ext_ime_config::Mode::Cangjie => Mode::Cangjie,
        cosmic_ext_ime_config::Mode::Quick => Mode::Quick,
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
