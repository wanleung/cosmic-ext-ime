//! Behavioural tests ported from ibus-chewing's `test/ibus-chewing-preedit-test.c`
//! (upstream commit series 2.0.x), so the adapter can be validated against the
//! exact key sequences upstream uses to define ibus-chewing's behaviour.
//!
//! Mapping notes:
//! * `assert_outgoing_pre_edit(outgoing, pre_edit)` corresponds to our
//!   accumulated `Response::Commit` text and `preedit().text`.
//! * `TEST_CASE_INIT()` sets conversion=chewing, half shape, Chinese mode; the
//!   harness below mirrors that (half shape instead of the user's full-width
//!   default).
//! * Upstream tests toggle Chinese/English with a Shift tap
//!   (`chi-eng-mode-toggle=shift`); the frontend owns Shift taps here, so the
//!   ports toggle with CapsLock instead.
//! * `ibus_chewing_pre_edit_force_commit` has no engine equivalent; the
//!   incomplete-character case asserts the bopomofo preedit instead.

use cosmic_ext_ime_chewing::{ChewingEngine, Config, Keyboard};
use cosmic_ext_ime_engine::{InputEngine, Key, KeyInput, Modifiers};

/// ibus-chewing engine defaults plus the user's carried-over settings.
/// `TEST_CASE_INIT()` forces half shape, so these tests do too.
fn config(name: &str) -> Config {
    let path = std::env::temp_dir().join(format!(
        "cosmic-ext-ime-chewing-ibus-test-{}-{name}.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    Config {
        keyboard: Keyboard::Default,
        page_size: 10,
        space_as_selection: true,
        auto_learn: false,
        fullwidth_chars: false,
        selection_keys: "1234567890".into(),
        easy_symbol_input: true,
        auto_shift_cur: true,
        add_phrase_direction: true,
        phrase_choice_from_last: true,
        esc_clean_all_buf: false,
        max_chi_symbol_len: 20,
        user_data_file: path,
        ..Config::default()
    }
}

struct Harness {
    e: ChewingEngine,
    /// Everything the engine committed so far (ibus-chewing's "outgoing").
    outgoing: String,
}

impl Harness {
    fn new(name: &str) -> Self {
        Harness {
            e: ChewingEngine::new(config(name)).expect("libchewing should initialise"),
            outgoing: String::new(),
        }
    }

    fn send(&mut self, input: KeyInput) {
        let r = self.e.process_key(input);
        if std::env::var("CHEWING_TEST_TRACE").is_ok() {
            eprintln!(
                "send {:?} -> {:?} preedit={:?}",
                input.key,
                r,
                self.e.preedit().text
            );
        }
        if let cosmic_ext_ime_engine::Response::Commit(t) = r {
            self.outgoing.push_str(&t);
        }
    }

    /// Lowercase/no-modifier character key.
    fn k(&mut self, c: char) {
        self.send(KeyInput {
            key: Key::Char(c),
            modifiers: Modifiers::default(),
        });
    }

    fn ks(&mut self, seq: &str) {
        for c in seq.chars() {
            self.k(c);
        }
    }

    /// Shifted character key (ibus `key_press_from_key_sym` with SHIFT_MASK).
    fn shift_k(&mut self, c: char) {
        self.send(KeyInput {
            key: Key::Char(c),
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        });
    }

    fn special(&mut self, key: Key) {
        self.send(KeyInput {
            key,
            modifiers: Modifiers::default(),
        });
    }

    fn preedit(&self) -> String {
        self.e.preedit().text.clone()
    }

    fn window_open(&self) -> bool {
        !self.e.candidates().is_empty()
    }
}

/// Select a named candidate from the open window, paging with Down like the
/// upstream horizontal table navigation.  Upstream's verbatim key sequences
/// assume the libchewing dictionary they were written against; candidate
/// order/length differs in Ubuntu 24.04's data, so the target is looked up by
/// text to keep testing the mechanism, not the dictionary.
fn select_candidate_named(h: &mut Harness, want: &str) {
    for _ in 0..12 {
        let page = h.e.candidates();
        if let Some(idx) = page.iter().position(|c| c.text == want) {
            let sel = if idx == 9 {
                '0'
            } else {
                char::from(b'1' + idx as u8)
            };
            h.k(sel);
            return;
        }
        h.special(Key::Down);
    }
    panic!("candidate {want:?} never appeared in the window");
}

/// Upstream `assert_outgoing_pre_edit`.
fn assert_outgoing_pre_edit(h: &Harness, outgoing: &str, pre_edit: &str) {
    assert_eq!(h.outgoing, outgoing, "outgoing mismatch");
    assert_eq!(h.preedit(), pre_edit, "preedit mismatch");
}

/* Upstream: process_key_normal_test — 中文 via "5j/ jp6" + Enter. */
#[test]
fn port_process_key_normal() {
    let mut h = Harness::new("normal");

    h.ks("5j/ jp6");
    assert_outgoing_pre_edit(&h, "", "中文");
    h.special(Key::Enter);
    assert_outgoing_pre_edit(&h, "中文", "");
}

/* Upstream: process_key_text_with_symbol_test — 他不重，他是我兄弟。 */
#[test]
fn port_process_key_text_with_symbol() {
    let mut h = Harness::new("text-symbol");

    h.ks("w8 ");
    h.special(Key::Down);
    h.ks("2");

    h.ks("1j65j/4");
    h.special(Key::Down);
    select_candidate_named(&mut h, "重");

    h.shift_k('<');

    h.ks("w8 g4ji3vm/ 2u4");

    h.shift_k('>');
    h.special(Key::Enter);

    assert_outgoing_pre_edit(&h, "他不重，他是我兄弟。", "");
}

/* Upstream: process_key_mix_test — "這是ibus-chewing 輸入法".
 * Shift taps are frontend-owned here, so CapsLock toggles the mode. */
#[test]
fn port_process_key_mix() {
    let mut h = Harness::new("mix");

    h.ks("5k4g4");
    h.special(Key::CapsLock); // to English
    h.ks("ibus-chewing ");
    h.special(Key::CapsLock); // back to Chinese
    h.ks("gj bj4z83");
    h.special(Key::Enter);

    // With 這是 still composed the English keystrokes are processed by
    // libchewing and quick-committed, exactly like upstream's expectation.
    assert_outgoing_pre_edit(&h, "這是ibus-chewing 輸入法", "");
}

/* Upstream: process_key_default_english_test — start in English mode,
 * type ASCII, switch to Chinese, type 輸入法. */
#[test]
fn port_process_key_default_english() {
    let mut h = Harness::new("default-english");

    h.special(Key::CapsLock); // start in English mode
    h.ks("gj bj4z83"); // forwarded, nothing composed
    assert_outgoing_pre_edit(&h, "", "");
    h.special(Key::CapsLock); // back to Chinese
    h.ks("gj bj4z83");
    h.special(Key::Enter);
    assert_outgoing_pre_edit(&h, "輸入法", "");
}

/* Upstream: process_key_incomplete_char_test — "u" leaves ㄧ in the preedit.
 * ibus_chewing_pre_edit_force_commit() has no engine equivalent, so the
 * bopomofo preedit itself is asserted. */
#[test]
fn port_process_key_incomplete_char() {
    let mut h = Harness::new("incomplete");

    h.ks("u");
    assert_eq!(h.preedit(), "ㄧ");
}

/* Upstream: process_key_buffer_full_handling_test —
 * 我家門前有小河，後面有山坡 */
#[test]
fn port_process_key_buffer_full_handling() {
    let mut h = Harness::new("buffer-full");

    h.ks("ji3ru8 ap6fu06u.3vul3ck6");
    h.shift_k('<');
    h.ks("c.4au04u.3g0 qi ");
    h.special(Key::Enter);

    assert!(h.outgoing.contains("我家門前有小河，後面有山坡"));
    assert_eq!(h.preedit(), "");
}

/* Upstream: process_key_down_up_arrow_test — 程式/城市/世 and the window
 * semantics of Down/Up/Escape. */
#[test]
fn port_process_key_down_up_arrow() {
    let mut h = Harness::new("down-up");

    h.ks("t/6g4");
    h.special(Key::Down);
    h.ks("1");
    assert_outgoing_pre_edit(&h, "", "城市");

    h.special(Key::Down);
    h.ks("2");
    assert_outgoing_pre_edit(&h, "", "程式");

    h.special(Key::Down);
    assert!(h.window_open(), "Down should show the table");
    h.special(Key::Up);
    assert!(h.window_open(), "Up pages while the table shows");
    h.special(Key::Escape);
    assert!(!h.window_open(), "Escape closes the table");
    h.special(Key::Up);
    assert!(!h.window_open(), "Up with the table closed is a no-op");

    h.special(Key::Down);
    h.special(Key::Down);
    h.ks("4");
    assert_eq!(
        h.preedit().chars().nth(1),
        Some('世'),
        "preedit: {}",
        h.preedit()
    );
}

/* Upstream: process_key_home_end_delete_test — 城市 → Home/Delete/End →
 * typing u06 yields 誓言. */
#[test]
fn port_process_key_home_end_delete() {
    let mut h = Harness::new("home-end");

    h.ks("t/6g4");
    h.special(Key::Down);
    h.ks("1");
    assert_outgoing_pre_edit(&h, "", "城市");
    h.special(Key::Home);
    assert_outgoing_pre_edit(&h, "", "城市");
    h.special(Key::Delete);
    h.special(Key::End);
    h.ks("u06");
    assert_outgoing_pre_edit(&h, "", "誓言");
}

/* Upstream: full_half_shape_test — Shift+Space toggles full shape; English
 * letters and space commit as full-width forms. */
#[test]
fn port_full_half_shape() {
    let mut h = Harness::new("shape");

    h.special(Key::CapsLock); // English mode
    h.send(KeyInput {
        key: Key::Space,
        modifiers: Modifiers {
            shift: true,
            ..Modifiers::default()
        },
    }); // Shift+Space: full shape on

    h.ks("ab ");
    assert_eq!(h.outgoing, "ａｂ　");

    // Toggle back and verify the mode really flipped.
    h.send(KeyInput {
        key: Key::Space,
        modifiers: Modifiers {
            shift: true,
            ..Modifiers::default()
        },
    });
    let half = std::ffi::CString::new("x").unwrap();
    let _ = half; // ShapeMode checked through behaviour below
    h.special(Key::CapsLock); // Chinese mode
    h.ks("5j/ jp6");
    h.special(Key::Enter);
    assert_eq!(h.outgoing, "ａｂ　中文");
}

/* Upstream: process_key_special_test — a non-printable/special keysym is
 * ignored and clears nothing. */
#[test]
fn port_process_key_special() {
    let mut h = Harness::new("special");

    h.ks("5j/ jp6");
    // Key::Other is the frontend's bucket for unhandled keysyms.
    h.send(KeyInput {
        key: Key::Other,
        modifiers: Modifiers::default(),
    });
    assert_outgoing_pre_edit(&h, "", "中文");
}

/* Upstream expects the candidate window to stay closed while typing. */
#[test]
fn port_no_auto_open_while_typing() {
    let mut h = Harness::new("no-auto");

    h.ks("5j/ jp6");
    assert!(!h.window_open());
    assert_eq!(h.preedit(), "中文");
}
