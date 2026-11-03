//! Additional use-case scenarios beyond `engine.rs` and
//! `ibus_chewing_parity.rs`: the remaining upstream ibus-chewing test cases
//! (space-as-selection, the backtick symbol menu, Ctrl+1, the keypad family)
//! plus adapter-level use cases (tones, cursor editing, mid-buffer selection,
//! repeated commits, shape toggling, forwarding rules).

use cosmic_ext_ime_chewing::{ChewingEngine, Config, Keyboard};
use cosmic_ext_ime_engine::{InputEngine, Key, KeyInput, Modifiers, Response};

fn harness(name: &str, fullwidth: bool) -> (ChewingEngine, String) {
    let path = std::env::temp_dir().join(format!(
        "cosmic-ext-ime-chewing-scenario-{}-{name}.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let cfg = Config {
        keyboard: Keyboard::Default,
        page_size: 10,
        space_as_selection: true,
        auto_learn: false,
        fullwidth_chars: fullwidth,
        selection_keys: "1234567890".into(),
        easy_symbol_input: true,
        auto_shift_cur: true,
        add_phrase_direction: true,
        phrase_choice_from_last: true,
        esc_clean_all_buf: false,
        max_chi_symbol_len: 20,
        user_data_file: path.clone(),
        ..Config::default()
    };
    (
        ChewingEngine::new(cfg).expect("libchewing should initialise"),
        path.to_string_lossy().into_owned(),
    )
}

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

fn shifted(c: char) -> KeyInput {
    KeyInput {
        key: Key::Char(c),
        modifiers: Modifiers {
            shift: true,
            ..Modifiers::default()
        },
    }
}

fn sp(key: Key) -> KeyInput {
    KeyInput {
        key,
        modifiers: Modifiers::default(),
    }
}

fn kp(d: u8) -> KeyInput {
    KeyInput {
        key: Key::KeypadDigit(d),
        modifiers: Modifiers::default(),
    }
}

fn kps(c: char) -> KeyInput {
    KeyInput {
        key: Key::KeypadSymbol(c),
        modifiers: Modifiers::default(),
    }
}

fn alt(c: char) -> KeyInput {
    KeyInput {
        key: Key::Char(c),
        modifiers: Modifiers {
            alt: true,
            ..Modifiers::default()
        },
    }
}

fn super_key(c: char) -> KeyInput {
    KeyInput {
        key: Key::Char(c),
        modifiers: Modifiers {
            logo: true,
            ..Modifiers::default()
        },
    }
}

fn type_str(e: &mut ChewingEngine, s: &str) {
    for c in s.chars() {
        e.process_key(key(c));
    }
}

/* ===== Remaining upstream test cases ===== */

/// Upstream test_space_as_selection (GitHub #79): Space with an empty buffer
/// commits a real space even when space-as-selection is on.
#[test]
fn space_with_empty_buffer_commits_space() {
    let (mut e, _p) = harness("space-empty", false);
    assert_eq!(
        e.process_key(sp(Key::Space)),
        Response::Commit(" ".to_string())
    );
    assert!(!e.is_composing());
}

/// Upstream test_ctrl_1_open_candidate_list (GitHub #63): Ctrl+1 opens the
/// symbol candidate list; Escape closes it.
#[test]
fn ctrl_1_opens_symbol_list() {
    let (mut e, _p) = harness("ctrl1", false);
    assert_eq!(e.process_key(ctrl('1')), Response::Consumed);
    assert!(
        !e.candidates().is_empty(),
        "Ctrl+1 should open the symbol list"
    );
    assert_eq!(e.process_key(sp(Key::Escape)), Response::Consumed);
    assert!(e.candidates().is_empty());
}

/// Upstream test_arrow_keys_buffer_empty (GitHub #50): the backtick opens the
/// symbol menu; arrows/page keys page it; Escape/Backspace dismiss it.
#[test]
fn backtick_menu_arrow_keys() {
    let (mut e, _p) = harness("backtick", false);
    assert_eq!(e.process_key(key('`')), Response::Consumed);
    assert!(!e.candidates().is_empty(), "` should open the symbol menu");
    assert_eq!(e.page_info().page, 0);

    // Horizontal lookup table: Right/Left move the highlight within the
    // page; PageUp/PageDown change pages.  (The upstream test enables the
    // vertical table, which we do not implement.)
    e.process_key(sp(Key::Right));
    assert_eq!(e.selected(), 1);
    e.process_key(sp(Key::Left));
    assert_eq!(e.selected(), 0);
    e.process_key(sp(Key::PageDown));
    assert_eq!(e.page_info().page, 1);
    e.process_key(sp(Key::PageUp));
    assert_eq!(e.page_info().page, 0);

    e.process_key(sp(Key::Escape));
    assert!(e.candidates().is_empty(), "Escape closes the menu");

    e.process_key(key('`'));
    assert!(!e.candidates().is_empty());
    e.process_key(sp(Key::Backspace));
    assert!(e.candidates().is_empty(), "Backspace closes the menu");
    assert!(!e.is_composing());
}

/// Upstream test_kp_eng_mode: keypad digits in English mode with an empty
/// buffer are forwarded.
#[test]
fn keypad_english_mode_forwarded() {
    let (mut e, _p) = harness("kp-eng", false);
    e.process_key(sp(Key::CapsLock));
    assert_eq!(e.process_key(kp(1)), Response::Ignored);
    assert_eq!(e.process_key(kp(9)), Response::Ignored);
    assert_eq!(e.process_key(kp(0)), Response::Ignored);
    assert!(!e.is_composing());
}

/// Upstream test_kp_chi_mode: keypad digits in Chinese mode with an empty
/// buffer are forwarded (the application receives the numbers).
#[test]
fn keypad_chinese_mode_empty_forwarded() {
    let (mut e, _p) = harness("kp-chi", false);
    assert_eq!(e.process_key(kp(1)), Response::Ignored);
    assert_eq!(e.process_key(kp(9)), Response::Ignored);
    assert_eq!(e.process_key(kp(0)), Response::Ignored);
    assert_eq!(e.preedit().text, "");
}

/// Upstream test_kp_chi_mode_buffer: with a non-empty buffer the keypad
/// digits go into the buffer via temporary English mode.
#[test]
fn keypad_appends_to_buffer() {
    let (mut e, _p) = harness("kp-buffer", false);
    type_str(&mut e, "su3cl3");
    assert_eq!(e.process_key(kp(1)), Response::Consumed);
    assert_eq!(e.process_key(kp(9)), Response::Consumed);
    assert_eq!(e.process_key(kp(0)), Response::Consumed);
    assert_eq!(e.preedit().text, "你好190");
    e.process_key(sp(Key::Enter));
    assert!(!e.is_composing());
}

/// Upstream test_kp_chi_incomplete: keypad keys clear in-progress bopomofo
/// (chewing_set_ChiEngMode clears the buffer on mode switches).
#[test]
fn keypad_clears_incomplete_bopomofo() {
    let (mut e, _p) = harness("kp-incomplete", false);
    type_str(&mut e, "su3cl");
    assert_eq!(e.process_key(kp(1)), Response::Consumed);
    assert_eq!(e.preedit().text, "你1");
}

/// Upstream test_kp_other_keys: keypad operators are appended half-width.
#[test]
fn keypad_operators_appended() {
    let (mut e, _p) = harness("kp-ops", false);
    type_str(&mut e, "su3cl3");
    for c in ['*', '+', ',', '-', '.', '/'] {
        e.process_key(kps(c));
    }
    assert_eq!(e.preedit().text, "你好*+,-./");
}

/// Upstream test_kp_selecting: Ctrl+KP-digit also opens the symbol list and
/// KP digits select from it.
#[test]
fn keypad_selecting_symbol_list() {
    let (mut e, _p) = harness("kp-select", false);
    let ctrl_kp1 = KeyInput {
        key: Key::KeypadDigit(1),
        modifiers: Modifiers {
            ctrl: true,
            ..Modifiers::default()
        },
    };
    assert_eq!(e.process_key(ctrl_kp1), Response::Consumed);
    assert!(!e.candidates().is_empty());
    assert_eq!(e.process_key(kp(2)), Response::Consumed);
    assert_eq!(e.preedit().text, "※");
    e.process_key(ctrl_kp1);
    assert!(!e.candidates().is_empty());
    e.process_key(kp(1));
    assert_eq!(e.preedit().text, "※…");
}

/* ===== New adapter use cases ===== */

/// Space completes a toneless syllable with the first tone.
#[test]
fn space_completes_first_tone() {
    let (mut e, _p) = harness("tone1", false);
    // 中 = ㄓㄨㄥˉ: the exact syllable upstream's own tests complete with
    // Space ("5j/ jp6" → 中文).  Tone 1 by Space only lands when the
    // syllable+tone is an actual word; ㄋㄧˉ is not, and libchewing discards
    // it (BOPOMOFO_NO_WORD).
    type_str(&mut e, "5j/");
    assert_eq!(e.preedit().text, "ㄓㄨㄥ");
    assert_eq!(e.process_key(sp(Key::Space)), Response::Consumed);
    assert_eq!(e.preedit().text, "中");
    assert!(e.is_composing());
}

/// Backspace deletes composed characters one by one.
#[test]
fn backspace_deletes_composed_chars() {
    let (mut e, _p) = harness("bs-chars", false);
    type_str(&mut e, "su3cl3");
    assert_eq!(e.preedit().text, "你好");
    e.process_key(sp(Key::Backspace));
    assert_eq!(e.preedit().text, "你");
    e.process_key(sp(Key::Backspace));
    assert_eq!(e.preedit().text, "");
    assert!(!e.is_composing());
}

/// Backspace removes in-progress bopomofo before touching the buffer.
#[test]
fn backspace_removes_bopomofo() {
    let (mut e, _p) = harness("bs-bpmf", false);
    e.process_key(key('s'));
    e.process_key(key('u'));
    assert_eq!(e.preedit().text, "ㄋㄧ");
    e.process_key(sp(Key::Backspace));
    assert_eq!(e.preedit().text, "ㄋ");
    e.process_key(sp(Key::Backspace));
    assert_eq!(e.preedit().text, "");
    assert!(!e.is_composing());
}

/// Delete with the cursor at Home removes the first character.
#[test]
fn delete_at_home_removes_first_char() {
    let (mut e, _p) = harness("del-home", false);
    type_str(&mut e, "su3cl3");
    e.process_key(sp(Key::Home));
    e.process_key(sp(Key::Delete));
    assert_eq!(e.preedit().text, "好");
    // Backspace at the Home position deletes nothing; move to the end first.
    e.process_key(sp(Key::End));
    e.process_key(sp(Key::Backspace));
    assert_eq!(e.preedit().text, "");
    // Delete on an empty buffer is forwarded to the application.
    assert_eq!(e.process_key(sp(Key::Delete)), Response::Ignored);
}

/// Home/End move the preedit cursor.
#[test]
fn home_end_move_preedit_cursor() {
    let (mut e, _p) = harness("cursor-move", false);
    type_str(&mut e, "su3cl3");
    assert_eq!(e.preedit().cursor, e.preedit().text.len());
    e.process_key(sp(Key::Home));
    assert_eq!(e.preedit().cursor, 0);
    e.process_key(sp(Key::End));
    assert_eq!(e.preedit().cursor, e.preedit().text.len());
}

/// Typing at Home inserts mid-buffer instead of appending.
#[test]
fn typing_at_home_inserts_mid_buffer() {
    let (mut e, _p) = harness("insert-mid", false);
    type_str(&mut e, "su3cl3");
    e.process_key(sp(Key::Home));
    type_str(&mut e, "su3");
    let text = e.preedit().text;
    assert_eq!(text.chars().count(), 3, "inserted before: {text}");
    assert!(text.starts_with('你'));
}

/// Selecting a single-char candidate mid-buffer replaces only that char.
#[test]
fn selection_mid_buffer_replaces_one_char() {
    let (mut e, _p) = harness("sel-mid", false);
    type_str(&mut e, "su3cl3");
    e.process_key(sp(Key::Home));
    e.process_key(sp(Key::Down));
    // Pick the first single-char candidate (phrase candidates at position 0
    // are single chars, but stay defensive).
    let page = e.candidates();
    let (idx, want) = page
        .iter()
        .enumerate()
        .find(|(_, c)| c.text.chars().count() == 1)
        .map(|(i, c)| (i, c.text.clone()))
        .expect("single-char candidate");
    let sel = if idx == 9 {
        '0'
    } else {
        char::from(b'1' + idx as u8)
    };
    e.process_key(key(sel));
    assert_eq!(e.preedit().text, format!("{want}好"));
}

/// The same character committed twice in a row is delivered twice: a commit
/// is read only when libchewing flags one for the key just handled.
#[test]
fn identical_commit_delivered_twice() {
    let (mut e, _p) = harness("dup-commit", false);
    let mut commits = String::new();
    for _ in 0..2 {
        type_str(&mut e, "su3");
        if let Response::Commit(t) = e.process_key(sp(Key::Enter)) {
            commits.push_str(&t);
        }
    }
    assert_eq!(commits, "你你");
}

/// Enter with an empty buffer is forwarded to the application.
#[test]
fn enter_empty_buffer_forwarded() {
    let (mut e, _p) = harness("enter-empty", false);
    assert_eq!(e.process_key(sp(Key::Enter)), Response::Ignored);
    // And again right after a commit.
    type_str(&mut e, "su3");
    e.process_key(sp(Key::Enter));
    assert_eq!(e.process_key(sp(Key::Enter)), Response::Ignored);
}

/// A second Escape after dismissing the window keeps the composition
/// (escCleanAllBuf is off).
#[test]
fn second_escape_keeps_composition() {
    let (mut e, _p) = harness("esc-twice", false);
    type_str(&mut e, "su3");
    e.process_key(sp(Key::Down));
    e.process_key(sp(Key::Escape));
    assert!(e.candidates().is_empty());
    assert_eq!(e.process_key(sp(Key::Escape)), Response::Consumed);
    assert_eq!(e.preedit().text, "你");
    assert!(e.is_composing());
}

/// CapsLock round-trip returns to Chinese and typing still works.
#[test]
fn capslock_roundtrip() {
    let (mut e, _p) = harness("caps-roundtrip", false);
    e.process_key(sp(Key::CapsLock));
    e.process_key(sp(Key::CapsLock));
    type_str(&mut e, "su3");
    assert_eq!(e.preedit().text, "你");
}

/// English mode with a non-empty buffer appends letters to the buffer.
#[test]
fn english_appends_to_buffer() {
    let (mut e, _p) = harness("eng-append", false);
    type_str(&mut e, "su3cl3");
    e.process_key(sp(Key::CapsLock));
    assert_eq!(e.process_key(key('a')), Response::Consumed);
    assert_eq!(e.preedit().text, "你好a");
    e.process_key(sp(Key::Enter));
    assert!(!e.is_composing());
}

/// Shift+Space toggles full shape mid-composition; easy symbols still insert.
#[test]
fn shift_space_toggles_shape_mid_composition() {
    let (mut e, _p) = harness("shape-mid", false);
    type_str(&mut e, "su3");
    let shift_space = KeyInput {
        key: Key::Space,
        modifiers: Modifiers {
            shift: true,
            ..Modifiers::default()
        },
    };
    assert_eq!(e.process_key(shift_space), Response::Consumed);
    assert_eq!(e.process_key(shifted('A')), Response::Consumed);
    let text = e.preedit().text;
    assert!(
        text.starts_with('你'),
        "composition kept after Shift+Space: {text}"
    );
}

/// PageDown then a selection key picks from the second page.
#[test]
fn page_down_selects_second_page() {
    let (mut e, _p) = harness("page-select", false);
    type_str(&mut e, "g4");
    e.process_key(sp(Key::Down));
    let total = e.page_info().total_pages.unwrap_or(1);
    if total < 2 {
        return; // dictionary without a second page: nothing to test
    }
    e.process_key(sp(Key::PageDown));
    assert_eq!(e.page_info().page, 1);
    let want = e.candidates()[0].text.clone();
    e.process_key(key('1'));
    assert_eq!(e.preedit().text, want);
}

/// Down past the last candidate page cycles the candidate list; the window
/// stays open.
#[test]
fn down_past_last_page_cycles() {
    let (mut e, _p) = harness("cycle", false);
    type_str(&mut e, "g4");
    e.process_key(sp(Key::Down));
    for _ in 0..12 {
        e.process_key(sp(Key::Down));
    }
    assert!(
        !e.candidates().is_empty(),
        "window stays open across cycles"
    );
    // libchewing rolls over to the next candidate list at the last page but
    // does not reset pageNo, so only require a consistent page.
    let info = e.page_info();
    assert!(info.page < info.total_pages.unwrap_or(0).max(1));
}

/// reset() while the candidate window is open clears everything.
#[test]
fn reset_while_selecting() {
    let (mut e, _p) = harness("reset-select", false);
    type_str(&mut e, "su3");
    e.process_key(sp(Key::Down));
    assert!(!e.candidates().is_empty());
    e.reset();
    assert!(e.candidates().is_empty());
    assert_eq!(e.preedit().text, "");
    assert!(!e.is_composing());
}

/// Tab without the candidate window toggles phrase segmentation and is
/// absorbed.
#[test]
fn tab_without_window_absorbed() {
    let (mut e, _p) = harness("tab-nowin", false);
    type_str(&mut e, "su3cl3");
    assert_eq!(e.process_key(sp(Key::Tab)), Response::Consumed);
    assert_eq!(e.preedit().text, "你好");
    assert!(e.is_composing());
}

/// Alt/Super combinations are forwarded to the application.
#[test]
fn alt_and_super_forwarded() {
    let (mut e, _p) = harness("alt-super", false);
    assert_eq!(e.process_key(alt('x')), Response::Ignored);
    assert_eq!(e.process_key(super_key('x')), Response::Ignored);
}

/// Non-ASCII characters are forwarded.
#[test]
fn non_ascii_forwarded() {
    let (mut e, _p) = harness("non-ascii", false);
    assert_eq!(e.process_key(key('é')), Response::Ignored);
    assert_eq!(e.process_key(key('你')), Response::Ignored);
}

/// Shifted letter goes through the easy-symbol table (Shift+A → 【).
#[test]
fn shifted_letter_uses_easy_symbol_table() {
    let (mut e, _p) = harness("easy-shift", false);
    assert_eq!(e.process_key(shifted('A')), Response::Consumed);
    assert_eq!(e.preedit().text, "【");
    e.process_key(sp(Key::Enter));
    assert_eq!(e.preedit().text, "");
}

/// ibus self_key_sym_fix parity: in Chinese mode the letter case comes from
/// Shift alone.  After a Caps Lock round-trip the physical caps state can be
/// ON while the engine is back in Chinese mode — uppercase keysyms must
/// still type bopomofo instead of dying silently.
#[test]
fn chinese_typing_survives_caps_lock_desync() {
    let (mut e, _p) = harness("caps-desync", false);
    // Toggle out and back: engine Chinese, XKB caps may still be locked,
    // so letter keysyms arrive uppercase without Shift.
    e.process_key(sp(Key::CapsLock));
    e.process_key(sp(Key::CapsLock));
    assert_eq!(e.process_key(key('S')), Response::Consumed);
    assert_eq!(e.preedit().text, "ㄋ");
    assert_eq!(e.process_key(key('U')), Response::Consumed);
    assert_eq!(e.preedit().text, "ㄋㄧ");
}

/// Shift+letter keeps using the easy-symbol table regardless of the raw
/// keysym case (caps+shift can deliver lowercase keysyms).
#[test]
fn shift_letter_easy_symbols_regardless_of_case() {
    let (mut e, _p) = harness("easy-case", false);
    // Lowercase keysym WITH Shift: normalised to uppercase -> easy table.
    assert_eq!(e.process_key(shifted('s')), Response::Consumed);
    assert_eq!(e.preedit().text, "】");
    e.process_key(sp(Key::Enter));
}

/// Upstream full_half_shape_test: with full shape ON, English-mode letters
/// commit as full-width forms (this is why shape stays OFF by default —
/// Caps Lock must type plain English).
#[test]
fn full_shape_english_commits_full_width() {
    let (mut e, _p) = harness("shape-eng", true);
    e.process_key(sp(Key::CapsLock)); // English mode, shape still on
    assert_eq!(e.process_key(key('a')), Response::Commit("ａ".to_string()));
    assert_eq!(e.process_key(key('b')), Response::Commit("ｂ".to_string()));
}

/// Default shape is half: English idle letters are forwarded half-width.
#[test]
fn english_idle_letters_forwarded() {
    let (mut e, _p) = harness("eng-idle", false);
    e.process_key(sp(Key::CapsLock));
    assert_eq!(e.process_key(key('z')), Response::Ignored);
    assert_eq!(e.process_key(key(' ')), Response::Ignored);
}

/// `set_english_mode`: the frontend's source switch resets the sub-mode.
/// English -> letters go to the application; Chinese -> bopomofo; a call
/// that matches the current mode is a no-op.
#[test]
fn set_english_mode_switches_sub_mode() {
    let (mut e, _p) = harness("set-eng", false);
    assert!(!e.set_english_mode(false), "already Chinese");
    assert!(e.set_english_mode(true));
    assert_eq!(
        e.process_key(key('s')),
        Response::Ignored,
        "English: forwarded"
    );
    assert!(!e.set_english_mode(true), "already English");
    assert!(e.set_english_mode(false));
    type_str(&mut e, "su3");
    assert_eq!(e.preedit().text, "你");
}

/// Caps Lock toggled the engine to English; a switch to the Chinese source
/// (frontend `set_enabled`) puts it back in Chinese whatever the key
/// history was.
#[test]
fn source_switch_resets_caps_lock_english() {
    let (mut e, _p) = harness("reset-eng", false);
    e.process_key(sp(Key::CapsLock)); // English
    assert_eq!(e.process_key(key('s')), Response::Ignored);
    assert!(e.set_english_mode(false));
    type_str(&mut e, "su3");
    assert_eq!(e.preedit().text, "你");
    // And the Caps Lock key keeps toggling from there.
    e.process_key(sp(Key::CapsLock));
    assert_eq!(
        e.process_key(key('a')),
        Response::Consumed,
        "non-empty buffer: appended"
    );
    assert_eq!(e.preedit().text, "你a");
}

/// Mid-syllable, a mode switch drops the incomplete bopomofo (like the Caps
/// Lock key does) and keeps the composed text.
#[test]
fn set_english_mode_drops_incomplete_syllable() {
    let (mut e, _p) = harness("set-eng-syl", false);
    type_str(&mut e, "su3c");
    assert_eq!(e.preedit().text, "你ㄏ");
    assert!(e.set_english_mode(true));
    assert_eq!(e.preedit().text, "你");
    assert!(e.is_composing());
    assert!(e.set_english_mode(false));
    type_str(&mut e, "cl3");
    assert_eq!(e.preedit().text, "你好");
}
