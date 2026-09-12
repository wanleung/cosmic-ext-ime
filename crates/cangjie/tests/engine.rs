use popeinput_cangjie::{CangjieEngine, Config, Mode};
use popeinput_engine::{InputEngine, Key, KeyInput, Modifiers, Response};

fn key(k: Key) -> KeyInput {
    KeyInput { key: k, modifiers: Modifiers::default() }
}

fn cangjie() -> CangjieEngine {
    CangjieEngine::new(Config::default()).unwrap()
}

fn quick() -> CangjieEngine {
    CangjieEngine::new(Config { mode: Mode::Quick, ..Config::default() }).unwrap()
}

fn type_str(engine: &mut CangjieEngine, s: &str) -> Vec<Response> {
    s.chars().map(|c| engine.process_key(key(Key::Char(c)))).collect()
}

#[test]
fn cangjie_waits_for_space_then_commits_unique_match() {
    let mut e = cangjie();
    type_str(&mut e, "ab");
    assert_eq!(e.preedit().text, "日月");
    assert!(e.candidates().is_empty());
    assert_eq!(e.process_key(key(Key::Space)), Response::Commit("明".into()));
    assert!(!e.is_composing());
}

#[test]
fn cangjie_multiple_matches_show_candidates_ranked_by_frequency() {
    let mut e = cangjie();
    type_str(&mut e, "a");
    assert_eq!(e.process_key(key(Key::Space)), Response::Consumed);
    let cands = e.candidates();
    assert!(cands.len() > 1);
    assert_eq!(cands[0].text, "日", "日 is the most frequent single-key 'a'");
    assert_eq!(e.process_key(key(Key::Char('1'))), Response::Commit("日".into()));
}

#[test]
fn space_commits_first_when_one_page_else_pages() {
    let mut e = cangjie();
    type_str(&mut e, "a");
    e.process_key(key(Key::Space));
    let info = e.page_info();
    let first = e.candidates()[0].text.clone();
    let r = e.process_key(key(Key::Space));
    if info.total_pages.unwrap() > 1 {
        assert_eq!(r, Response::Consumed);
        assert_eq!(e.page_info().page, 1);
    } else {
        assert_eq!(r, Response::Commit(first));
    }
}

#[test]
fn typing_new_key_auto_commits_first_candidate() {
    let mut e = cangjie();
    type_str(&mut e, "a");
    e.process_key(key(Key::Space));
    let first = e.candidates()[0].text.clone();
    assert_eq!(e.process_key(key(Key::Char('b'))), Response::Commit(first));
    assert_eq!(e.preedit().text, "月");
}

#[test]
fn quick_looks_up_first_and_last_key() {
    let mut e = quick();
    type_str(&mut e, "a");
    assert!(e.candidates().is_empty());
    let r = e.process_key(key(Key::Char('b')));
    assert_eq!(r, Response::Consumed);
    assert_eq!(e.preedit().text, "日月");
    let cands = e.candidates();
    assert!(!cands.is_empty());
    assert!(cands.len() <= 9);
    assert_eq!(cands[0].text, "明", "明 (日月) must rank first for Quick ab");
    assert_eq!(cands[0].hint.as_deref(), Some("日月"));
    assert!(cands.iter().any(|c| c.text == "晴"), "晴 (日手一月) matches a*b");
}

#[test]
fn quick_third_key_commits_first_and_starts_over() {
    let mut e = quick();
    type_str(&mut e, "ab");
    assert_eq!(e.process_key(key(Key::Char('c'))), Response::Commit("明".into()));
    assert_eq!(e.preedit().text, "金");
    assert!(e.candidates().is_empty());
}

#[test]
fn quick_unknown_pair_rings_bell_and_clears_on_next_key() {
    let mut e = quick();
    type_str(&mut e, "z");
    let r = e.process_key(key(Key::Char('z')));
    assert_eq!(r, Response::Bell);
    assert_eq!(e.preedit().text, "ＺＺ", "bad input stays visible");
    type_str(&mut e, "a");
    assert_eq!(e.preedit().text, "日");
}

#[test]
fn escape_and_backspace() {
    let mut e = cangjie();
    type_str(&mut e, "abc");
    assert_eq!(e.process_key(key(Key::Backspace)), Response::Consumed);
    assert_eq!(e.preedit().text, "日月");
    assert_eq!(e.process_key(key(Key::Escape)), Response::Consumed);
    assert!(!e.is_composing());
    assert_eq!(e.process_key(key(Key::Escape)), Response::Ignored);
    assert_eq!(e.process_key(key(Key::Backspace)), Response::Ignored);
}

#[test]
fn fullwidth_punctuation_from_shortcode_table() {
    let mut e = cangjie();
    let r = e.process_key(key(Key::Char(',')));
    match r {
        Response::Commit(t) => assert_ne!(t, ","),
        Response::Consumed => {
            let cands = e.candidates();
            assert!(cands.iter().any(|c| c.text == "，"));
            assert_eq!(e.preedit().text, ",");
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn fullwidth_space_and_digits() {
    let mut e = cangjie();
    assert_eq!(e.process_key(key(Key::Space)), Response::Commit("　".into()));
    assert_eq!(e.process_key(key(Key::Char('1'))), Response::Commit("１".into()));
}

#[test]
fn halfwidth_mode_forwards_punctuation() {
    let mut e = CangjieEngine::new(Config { fullwidth_chars: false, ..Config::default() }).unwrap();
    assert_eq!(e.process_key(key(Key::Space)), Response::Ignored);
    assert_eq!(e.process_key(key(Key::Char(',')), ), Response::Ignored);
    assert_eq!(e.process_key(key(Key::Char('1'))), Response::Ignored);
}

#[test]
fn punctuation_after_input_commits_then_handles_punctuation() {
    let mut e = cangjie();
    type_str(&mut e, "ab");
    let r = e.process_key(key(Key::Char(',')));
    match r {
        Response::Commit(t) => assert!(t.starts_with('明'), "got {t:?}"),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn halfwidth_punctuation_after_input_commits_and_forwards() {
    let mut e = CangjieEngine::new(Config { fullwidth_chars: false, ..Config::default() }).unwrap();
    type_str(&mut e, "ab");
    assert_eq!(e.process_key(key(Key::Char(','))), Response::CommitAndForward("明".into()));
    assert!(!e.is_composing());
}

#[test]
fn modifiers_pass_through() {
    let mut e = cangjie();
    let ctrl_a = KeyInput {
        key: Key::Char('a'),
        modifiers: Modifiers { ctrl: true, ..Modifiers::default() },
    };
    assert_eq!(e.process_key(ctrl_a), Response::Ignored);
    assert!(!e.is_composing());
}

#[test]
fn wildcard_in_cangjie() {
    let mut e = cangjie();
    type_str(&mut e, "a*b");
    assert_eq!(e.preedit().text, "日＊月");
    assert_eq!(e.process_key(key(Key::Space)), Response::Consumed);
    assert!(e.candidates().iter().any(|c| c.text == "晴"));
}

#[test]
fn enter_is_forwarded() {
    let mut e = cangjie();
    type_str(&mut e, "ab");
    assert_eq!(e.process_key(key(Key::Enter)), Response::Ignored);
}
