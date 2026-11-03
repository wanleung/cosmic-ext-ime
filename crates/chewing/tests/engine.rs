use cosmic_ext_ime_chewing::{ChewingEngine, Config, Keyboard};
use cosmic_ext_ime_engine::{InputEngine, Key, KeyInput, Modifiers, Response};

fn key(c: char) -> KeyInput {
    KeyInput {
        key: Key::Char(c),
        modifiers: Modifiers::default(),
    }
}

fn ctrl(c: char) -> KeyInput {
    KeyInput {
        key: Key::Char(c),
        modifiers: Modifiers {
            ctrl: true,
            ..Modifiers::default()
        },
    }
}

fn special(key: Key) -> KeyInput {
    KeyInput {
        key,
        modifiers: Modifiers::default(),
    }
}

fn config(name: &str, esc_clean_all_buf: bool) -> Config {
    let path = std::env::temp_dir().join(format!(
        "cosmic-ext-ime-chewing-test-{}-{name}.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    Config {
        keyboard: Keyboard::Default,
        page_size: 10,
        space_as_selection: true,
        auto_learn: false,
        fullwidth_chars: true,
        selection_keys: "1234567890".into(),
        easy_symbol_input: true,
        auto_shift_cur: true,
        add_phrase_direction: true,
        phrase_choice_from_last: true,
        esc_clean_all_buf,
        max_chi_symbol_len: 20,
        user_data_file: path,
        ..Config::default()
    }
}

fn engine(name: &str) -> ChewingEngine {
    ChewingEngine::new(config(name, false)).expect("libchewing should initialise")
}

fn type_str(e: &mut ChewingEngine, s: &str) {
    for c in s.chars() {
        e.process_key(key(c));
    }
}

#[test]
fn first_zhuyin_keys_are_consumed() {
    let mut e = engine("consumed");

    // The in-progress syllable lives in libchewing's bopomofo buffer, not in
    // chewing_buffer_String.  These keys must not reach the application.
    assert_eq!(e.process_key(key('s')), Response::Consumed);
    assert_eq!(e.preedit().text, "ㄋ");
    assert_eq!(e.process_key(key('u')), Response::Consumed);
    assert_eq!(e.preedit().text, "ㄋㄧ");
    assert_eq!(e.process_key(key('3')), Response::Consumed);
    assert_eq!(e.preedit().text, "你");
}

#[test]
fn candidate_window_does_not_auto_open() {
    let mut e = engine("no-auto-open");

    type_str(&mut e, "su3");
    assert!(e.is_composing());
    assert_eq!(e.preedit().text, "你");
    // ibus-chewing only shows the candidate window on demand (Down/Space).
    assert!(e.candidates().is_empty());
    assert_eq!(e.page_info().total_pages, Some(0));
}

#[test]
fn down_opens_window_and_selection_replaces_preedit() {
    let mut e = engine("select");

    type_str(&mut e, "su3");
    assert_eq!(e.process_key(special(Key::Down)), Response::Consumed);
    assert!(!e.candidates().is_empty(), "window should be open");
    assert!(e.page_info().total_pages.unwrap_or(0) > 0);

    // ibus-chewing puts the chosen candidate into the preedit; the buffer is
    // committed with Enter, not by the selection itself.
    assert_eq!(e.process_key(key('1')), Response::Consumed);
    assert_eq!(e.preedit().text, "你");
    assert_eq!(
        e.process_key(special(Key::Enter)),
        Response::Commit("你".to_string())
    );
    assert!(!e.is_composing());
}

#[test]
fn space_opens_window_like_down() {
    let mut e = engine("space-open");

    type_str(&mut e, "su3");
    assert_eq!(e.process_key(special(Key::Space)), Response::Consumed);
    assert!(!e.candidates().is_empty());
}

#[test]
fn selects_a_different_candidate() {
    let mut e = engine("different");

    type_str(&mut e, "g4");
    e.process_key(special(Key::Down));
    assert_eq!(e.preedit().text, "市");
    e.process_key(key('2'));
    assert_eq!(e.preedit().text, "是");
    assert_eq!(
        e.process_key(special(Key::Enter)),
        Response::Commit("是".to_string())
    );
}

#[test]
fn composes_multiple_syllables_without_window() {
    let mut e = engine("multi");

    // 你好: su3 cl3 — typing continues while the window stays closed.
    type_str(&mut e, "su3cl3");
    assert_eq!(e.preedit().text, "你好");
    assert!(e.candidates().is_empty());

    assert_eq!(
        e.process_key(special(Key::Enter)),
        Response::Commit("你好".to_string())
    );
    assert!(!e.is_composing());
}

#[test]
fn shift_punctuation_commits_full_width() {
    let mut e = engine("punct");
    // Shifted keys map to full-width punctuation immediately.
    assert_eq!(e.process_key(key('<')), Response::Commit("，".to_string()));
    assert_eq!(e.process_key(key('>')), Response::Commit("。".to_string()));
}

#[test]
fn unshifted_punctuation_types_bopomofo() {
    let mut e = engine("bopomofo-punct");
    // Da Chen layout: unshifted , and . are the Zhuyin keys ㄝ and ㄡ.
    assert_eq!(e.process_key(key(',')), Response::Consumed);
    assert_eq!(e.preedit().text, "ㄝ");
    e.reset();
    assert_eq!(e.process_key(key('.')), Response::Consumed);
    assert_eq!(e.preedit().text, "ㄡ");
}

#[test]
fn right_moves_highlight_and_enter_selects_it() {
    let mut e = engine("cursor");
    type_str(&mut e, "g4");
    e.process_key(special(Key::Down));
    assert_eq!(e.selected(), 0);
    assert_eq!(e.process_key(special(Key::Right)), Response::Consumed);
    assert_eq!(e.selected(), 1);
    e.process_key(special(Key::Enter));
    assert_eq!(e.preedit().text, "是");
}

#[test]
fn left_moves_highlight_back() {
    let mut e = engine("cursor-back");
    type_str(&mut e, "g4");
    e.process_key(special(Key::Down));
    e.process_key(special(Key::Right));
    e.process_key(special(Key::Right));
    assert_eq!(e.selected(), 2);
    assert_eq!(e.process_key(special(Key::Left)), Response::Consumed);
    assert_eq!(e.selected(), 1);
}

#[test]
fn tab_is_absorbed_while_window_open() {
    let mut e = engine("tab");
    type_str(&mut e, "g4");
    e.process_key(special(Key::Down));
    // ibus-chewing: Tab does not move the candidate highlight.
    assert_eq!(e.process_key(special(Key::Tab)), Response::Consumed);
    assert_eq!(e.selected(), 0);
}

#[test]
fn space_pages_forward_while_window_open() {
    let mut e = engine("space-page");
    type_str(&mut e, "g4");
    e.process_key(special(Key::Down));
    assert_eq!(e.page_info().page, 0);
    assert_eq!(e.process_key(special(Key::Space)), Response::Consumed);
    assert!(e.page_info().page > 0, "space should advance the page");
}

#[test]
fn letters_are_absorbed_while_window_open() {
    let mut e = engine("absorb");
    type_str(&mut e, "su3");
    e.process_key(special(Key::Down));
    assert_eq!(e.process_key(key('x')), Response::Consumed);
    assert_eq!(e.preedit().text, "你");
    assert!(!e.candidates().is_empty(), "window stays open");
}

#[test]
fn backspace_dismisses_window_and_keeps_composition() {
    let mut e = engine("backspace-dismiss");
    type_str(&mut e, "su3");
    e.process_key(special(Key::Down));
    assert!(!e.candidates().is_empty());
    assert_eq!(e.process_key(special(Key::Backspace)), Response::Consumed);
    assert!(e.candidates().is_empty(), "window closed");
    assert_eq!(e.preedit().text, "你", "composition kept");
    assert!(e.is_composing());
}

#[test]
fn escape_dismisses_window_and_keeps_composition() {
    let mut e = engine("esc-dismiss");
    type_str(&mut e, "su3");
    e.process_key(special(Key::Down));
    assert_eq!(e.process_key(special(Key::Escape)), Response::Consumed);
    assert!(e.candidates().is_empty());
    assert_eq!(e.preedit().text, "你");
    assert!(e.is_composing());
}

#[test]
fn escape_clears_bopomofo_without_window() {
    let mut e = engine("esc-bopomofo");
    e.process_key(key('s'));
    assert_eq!(e.preedit().text, "ㄋ");
    assert_eq!(e.process_key(special(Key::Escape)), Response::Consumed);
    assert!(e.preedit().text.is_empty());
    assert!(!e.is_composing());
}

#[test]
fn escape_clears_everything_when_esc_clean_all_buf() {
    let mut e = ChewingEngine::new(config("esc-clean", true)).expect("init");
    type_str(&mut e, "su3");
    e.process_key(special(Key::Escape)); // nothing to clean yet, closes window
    type_str(&mut e, "cl3");
    assert_eq!(e.preedit().text, "好");
    assert_eq!(e.process_key(special(Key::Escape)), Response::Consumed);
    assert!(
        e.preedit().text.is_empty(),
        "escCleanAllBuf clears the buffer"
    );
    assert!(!e.is_composing());
}

#[test]
fn ctrl_num_adds_user_phrase_while_composing() {
    let mut e = engine("ctrlnum");
    type_str(&mut e, "su3");
    assert_eq!(e.process_key(key('3')), Response::Consumed);
    assert_eq!(e.process_key(ctrl('2')), Response::Consumed);
    assert!(e.is_composing());
}

#[test]
fn ctrl_num_is_absorbed_while_idle_in_chinese_mode() {
    let mut e = engine("ctrlnum-idle");
    assert_eq!(e.process_key(ctrl('5')), Response::Consumed);
}

#[test]
fn ctrl_num_is_forwarded_in_english_mode() {
    let mut e = engine("ctrlnum-english");
    e.process_key(special(Key::CapsLock));
    assert_eq!(e.process_key(ctrl('5')), Response::Ignored);
}

#[test]
fn zero_selects_tenth_candidate() {
    let mut e = engine("zero");
    type_str(&mut e, "g4");
    e.process_key(special(Key::Down));
    let expected = e.candidates()[9].text.clone();
    e.process_key(key('0'));
    assert_eq!(e.preedit().text, expected);
}

#[test]
fn j_moves_window_to_previous_char() {
    let path = std::env::temp_dir().join(format!(
        "cosmic-ext-ime-chewing-test-{}-jk.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let mut e = ChewingEngine::new(Config {
        phrase_choice_from_last: false,
        ..config("jk", false)
    })
    .expect("libchewing should initialise");
    type_str(&mut e, "su3cl3");
    e.process_key(special(Key::Down));
    assert!(!e.candidates().is_empty());
    let before = e.candidates();
    // libchewing 0.6: j/k move the candidate window to the neighbouring char.
    assert_eq!(e.process_key(key('j')), Response::Consumed);
    assert!(!e.candidates().is_empty());
    let after = e.candidates();
    assert_ne!(
        before, after,
        "window should now list the previous syllable"
    );
}

#[test]
fn uppercase_keysym_without_shift_types_bopomofo() {
    let mut e = engine("fullwidth-letter");
    // A physical Caps Lock delivers uppercase keysyms with no Shift; in
    // Chinese mode the case comes from Shift alone (ibus self_key_sym_fix),
    // so this types bopomofo instead of a full-width latin letter.
    assert_eq!(e.process_key(key('A')), Response::Consumed);
    assert_eq!(e.preedit().text, "ㄇ");
}

#[test]
fn shift_arrow_selects_range_and_enter_adds_phrase() {
    let mut e = engine("phrase-select");
    type_str(&mut e, "su3cl3");
    assert_eq!(e.preedit().text, "你好");

    let shift_left = KeyInput {
        key: Key::Left,
        modifiers: Modifiers {
            shift: true,
            ..Modifiers::default()
        },
    };
    // ibus-chewing: Shift+Left extends a phrase selection; Enter then adds
    // the selected range as a user phrase instead of committing.
    assert_eq!(e.process_key(shift_left), Response::Consumed);
    let response = e.process_key(special(Key::Enter));
    assert_eq!(response, Response::Consumed);
    assert!(e.is_composing(), "add-phrase keeps the composition");
}

#[test]
fn reset_clears_composition() {
    let mut e = engine("reset");
    type_str(&mut e, "su3");
    assert!(e.is_composing());
    e.reset();
    assert!(!e.is_composing());
    assert!(e.preedit().text.is_empty());
    assert!(e.candidates().is_empty());
}

fn plain_engine(name: &str) -> ChewingEngine {
    let cfg = Config {
        plain_zhuyin: true,
        ..config(name, false)
    };
    ChewingEngine::new(cfg).expect("libchewing should initialise")
}

/// Upstream plain_zhuyin_test: a completed syllable opens the candidate
/// window at once, the preedit shows the first candidate, and a selection
/// key commits that character and closes the window.
#[test]
fn plain_zhuyin_syllable_opens_window_and_selection_commits() {
    let mut e = plain_engine("plain-open");
    type_str(&mut e, "y ");
    let cands = e.candidates();
    assert!(
        !cands.is_empty(),
        "window should be open after the syllable"
    );
    assert_eq!(e.preedit().text, cands[0].text);
    let fourth = cands[3].text.clone();
    assert_eq!(e.process_key(key('4')), Response::Commit(fourth));
    assert!(e.candidates().is_empty(), "window closes after the pick");
    assert!(!e.is_composing());
}

/// Upstream plain_zhuyin_shift_symbol_test: "su31cl31" commits 你 and 好
/// one at a time; Shift+, then commits full-width punctuation directly.
#[test]
fn plain_zhuyin_select_first_each_syllable() {
    let mut e = plain_engine("plain-first");
    type_str(&mut e, "su3");
    assert_eq!(e.process_key(key('1')), Response::Commit("你".into()));
    type_str(&mut e, "cl3");
    assert_eq!(e.process_key(key('1')), Response::Commit("好".into()));
    assert!(!e.is_composing());
    let comma = KeyInput {
        key: Key::Char('<'),
        modifiers: Modifiers {
            shift: true,
            ..Modifiers::default()
        },
    };
    assert_eq!(e.process_key(comma), Response::Commit("，".into()));
}

/// Typing the next syllable while the window is open accepts the current
/// character (commits it) and starts the new syllable — like libchewing's
/// SimpleEngine retrying the key as syllable input.
#[test]
fn plain_zhuyin_next_syllable_accepts_current() {
    let mut e = plain_engine("plain-next");
    type_str(&mut e, "su3");
    assert!(!e.candidates().is_empty());
    assert_eq!(e.process_key(key('c')), Response::Commit("你".into()));
    assert_eq!(e.preedit().text, "ㄏ");
    type_str(&mut e, "l3");
    assert_eq!(e.preedit().text, "好");
    assert_eq!(
        e.process_key(special(Key::Enter)),
        Response::Commit("好".into())
    );
}

/// Backspace dismisses the window and keeps the character; continuing to
/// type commits it first, so the buffer never grows past one character.
#[test]
fn plain_zhuyin_backspace_keeps_then_next_key_commits() {
    let mut e = plain_engine("plain-bs");
    type_str(&mut e, "su3");
    assert_eq!(e.process_key(special(Key::Backspace)), Response::Consumed);
    assert!(e.candidates().is_empty());
    assert_eq!(e.preedit().text, "你");
    assert_eq!(e.process_key(key('c')), Response::Commit("你".into()));
    assert_eq!(e.preedit().text, "ㄏ");
}

/// Escape with the window open throws the character away (SimpleEngine).
#[test]
fn plain_zhuyin_escape_discards_character() {
    let mut e = plain_engine("plain-esc");
    type_str(&mut e, "su3");
    assert_eq!(e.process_key(special(Key::Escape)), Response::Consumed);
    assert!(!e.is_composing());
    assert_eq!(e.preedit().text, "");
}

/// Plain mode off: nothing changes (the window still only opens on demand).
#[test]
fn plain_zhuyin_off_keeps_phrase_conversion() {
    let mut e = engine("plain-off");
    type_str(&mut e, "su3cl3");
    assert!(e.candidates().is_empty());
    assert_eq!(e.preedit().text, "你好");
}

/// Enter commits the character shown in the preedit, which is the
/// conversion's choice and not necessarily the list's first entry.
#[test]
fn plain_zhuyin_enter_commits_displayed_character() {
    let mut e = plain_engine("plain-enter");
    type_str(&mut e, "g4");
    let shown = e.preedit().text.clone();
    assert!(!e.candidates().is_empty());
    assert_eq!(e.process_key(special(Key::Enter)), Response::Commit(shown));
    assert!(!e.is_composing());
}

/// Enter on a moved highlight commits that candidate instead.
#[test]
fn plain_zhuyin_enter_commits_moved_highlight() {
    let mut e = plain_engine("plain-enter-right");
    type_str(&mut e, "su3");
    let second = e.candidates()[1].text.clone();
    assert_eq!(e.process_key(special(Key::Right)), Response::Consumed);
    assert_eq!(e.process_key(special(Key::Enter)), Response::Commit(second));
}

/// Left/Right move the editing cursor inside the composition (ibus
/// self_handle_left/right): a syllable typed after Left lands before the
/// character the cursor passed.
#[test]
fn arrows_move_the_composition_cursor() {
    let mut e = engine("arrows-cursor");
    type_str(&mut e, "su3cl3");
    assert_eq!(e.preedit().text, "你好");
    assert_eq!(e.process_key(special(Key::Left)), Response::Consumed);
    type_str(&mut e, "ji3");
    assert_eq!(e.preedit().text, "你我好");
    assert_eq!(e.process_key(special(Key::Right)), Response::Consumed);
    type_str(&mut e, "a87");
    assert_eq!(e.preedit().text, "你我好嗎");
}
