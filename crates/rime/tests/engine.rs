use cosmic_ext_ime_engine::{InputEngine, Key, KeyInput, Modifiers, Response};
use cosmic_ext_ime_rime::{Config, RimeEngine};

fn key(k: Key) -> KeyInput {
    KeyInput {
        key: k,
        modifiers: Modifiers::default(),
    }
}

fn config(schema: &str) -> Config {
    let dir = std::env::temp_dir().join(format!("cosmic-ext-ime-rime-test-{}", std::process::id()));
    Config {
        user_data_dir: dir,
        schema: Some(schema.to_string()),
        ..Config::default()
    }
}

#[test]
fn cangjie5_schema_composes_and_commits() {
    if !std::path::Path::new("/usr/share/rime-data/cangjie5.schema.yaml").exists() {
        eprintln!("rime-data-cangjie5 not installed; skipping");
        return;
    }
    let mut e = RimeEngine::new(config("cangjie5")).expect("rime engine");
    assert!(
        e.name().contains("倉頡") || e.name().to_lowercase().contains("cangjie"),
        "name {:?}",
        e.name()
    );

    assert_eq!(e.process_key(key(Key::Char('a'))), Response::Consumed);
    assert_eq!(e.process_key(key(Key::Char('b'))), Response::Consumed);
    assert!(e.is_composing());
    assert!(
        e.preedit().text.contains("日月"),
        "preedit {:?}",
        e.preedit().text
    );
    let cands = e.candidates();
    assert!(cands.iter().any(|c| c.text == "明"), "candidates {cands:?}");
    assert!(cands.len() <= e.page_info().page_size);

    let r = e.process_key(key(Key::Space));
    match r {
        Response::Commit(t) => assert!(t.contains('明'), "committed {t:?}"),
        other => panic!("unexpected {other:?}"),
    }
    assert!(!e.is_composing());

    // Escape with nothing composing is not RIME's business.
    assert_eq!(e.process_key(key(Key::Escape)), Response::Ignored);

    assert!(!cosmic_ext_ime_rime::schemas().is_empty());
    drop(e);
    let _ = std::fs::remove_dir_all(config("cangjie5").user_data_dir);
}
