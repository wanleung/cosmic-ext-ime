use cosmic_ext_ime_engine::{InputEngine, Key, KeyInput, Modifiers, Response};
use cosmic_ext_ime_pinyin::{Config, PinyinEngine, Scheme};
use std::path::PathBuf;
use std::sync::Mutex;

/// libpinyin's context is process-global and single-threaded.
static SERIAL: Mutex<()> = Mutex::new(());

fn key(k: Key) -> KeyInput {
    KeyInput {
        key: k,
        modifiers: Modifiers::default(),
    }
}

fn config() -> Config {
    let mut cfg = Config::default();
    if let Some(dir) = std::env::var_os("LIBPINYIN_DATA_DIR") {
        cfg.system_data_dir = PathBuf::from(dir);
    }
    cfg.user_data_dir =
        std::env::temp_dir().join(format!("cosmic-ext-ime-pinyin-test-{}", std::process::id()));
    cfg
}

fn type_str(e: &mut PinyinEngine, s: &str) {
    for c in s.chars() {
        e.process_key(key(Key::Char(c)));
    }
}

#[test]
fn sentence_prediction_and_selection() {
    let _guard = SERIAL.lock().unwrap();
    let cfg = config();
    if !cfg.system_data_dir.join("merged.bin").exists() {
        eprintln!("libpinyin data not found; skipping");
        return;
    }
    let mut e = PinyinEngine::new(cfg.clone()).expect("pinyin engine");
    assert_eq!(e.name(), "拼音");

    type_str(&mut e, "nihao");
    assert!(e.is_composing());
    assert_eq!(e.preedit().text, "你好", "sentence guess for nihao");
    assert!(
        e.header().starts_with("ni"),
        "header shows segmented pinyin: {:?}",
        e.header()
    );
    let cands = e.candidates();
    assert_eq!(cands[0].text, "你好");
    assert!(
        cands.iter().any(|c| c.text == "你"),
        "single-word candidates offered too"
    );

    // Space commits the whole guessed sentence.
    assert_eq!(
        e.process_key(key(Key::Space)),
        Response::Commit("你好".into())
    );
    assert!(!e.is_composing());

    // Word-by-word: pick 你 first, then the rest.
    type_str(&mut e, "nihao");
    let idx = e.candidates().iter().position(|c| c.text == "你").unwrap();
    assert!(idx < 9);
    let r = e.process_key(key(Key::Char(char::from(b'1' + idx as u8))));
    assert_eq!(
        r,
        Response::Consumed,
        "selecting a word continues composing"
    );
    assert_eq!(e.preedit().text, "你好");
    // The full sentence stays first (picking it commits the rest); the
    // remaining syllable's words follow.
    let cands = e.candidates();
    assert_eq!(cands[0].text, "你好");
    assert!(cands.iter().any(|c| c.text == "好"), "{cands:?}");
    assert!(
        !cands.iter().any(|c| c.text == "你"),
        "already-chosen word not offered again"
    );
    // Backspace undoes the selection, not the pinyin.
    assert_eq!(e.process_key(key(Key::Backspace)), Response::Consumed);
    assert!(e.candidates().iter().any(|c| c.text == "你"));
    // Then Backspace removes letters.
    e.process_key(key(Key::Backspace));
    assert!(e.header().starts_with("ni"), "{:?}", e.header());

    // Escape clears; Enter commits raw pinyin.
    e.process_key(key(Key::Escape));
    assert!(!e.is_composing());
    type_str(&mut e, "abc");
    assert_eq!(
        e.process_key(key(Key::Enter)),
        Response::Commit("abc".into())
    );

    // Full-width punctuation while idle, and after a sentence.
    assert_eq!(
        e.process_key(key(Key::Char(','))),
        Response::Commit("，".into())
    );
    type_str(&mut e, "nihao");
    assert_eq!(
        e.process_key(key(Key::Char('.'))),
        Response::Commit("你好。".into())
    );

    drop(e);
    let _ = std::fs::remove_dir_all(cfg.user_data_dir);
}

#[test]
fn double_pinyin_scheme() {
    let _guard = SERIAL.lock().unwrap();
    let cfg = Config {
        scheme: Scheme::DoubleZrm,
        ..config()
    };
    if !cfg.system_data_dir.join("merged.bin").exists() {
        return;
    }
    let mut e = PinyinEngine::new(cfg).expect("pinyin engine");
    assert_eq!(e.name(), "双拼");
    // 自然码: ni + hk (hao)
    type_str(&mut e, "nihk");
    assert_eq!(e.preedit().text, "你好", "header {:?}", e.header());
}
