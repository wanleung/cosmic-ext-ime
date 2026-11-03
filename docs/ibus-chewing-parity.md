# ibus-chewing parity checklist

Reference: `ibus-chewing` 2.0 key rules (`src/ibus-chewing-preedit.c`) and the
`org.freedesktop.IBus.Chewing` GSettings schema.

## Status

| Area | ibus-chewing behaviour | cosmic-ext-ime |
|---|---|---|
| Bopomofo preedit | in-progress syllable visible and consumed | done |
| Multi-syllable | continuous composition | done |
| Candidate list | current page, selection cursor | done |
| Candidate keys | Tab/Shift+Tab, arrows, PageUp/Down, j/k, Enter/Space, 0/custom keys | done for cached list; dismissal semantics still partial |
| Candidate dismissal | Esc/Up/Backspace close list | partial |
| Full-width shape | Shift+Space, Shift+A, punctuation, easy symbols | partial: shape mode + idle punctuation; composing/easy symbols missing |
| Chinese/English | Caps Lock toggle, sync-caps, Shift tap | Caps Lock toggle (the compositor's Lock is cleared again: lower-case English), Shift tap; sync-caps deliberately not followed — source switch resets to Chinese instead |
| Editing | Backspace, Delete, Home/End, cursor movement | done for Delete/Home/End; preedit cursor movement still partial |
| Phrase selection | Shift+Left/Right, add-phrase direction | missing |
| Ctrl+Num | symbol table / phrase shortcuts | missing |
| Plain Zhuyin | simple mode, no intelligent phrasing | missing |
| Settings | kb-type, cand/page, sel-keys, space-select, plain, easy-symbol, auto-shift, phrase direction, esc-clean, max length, chi-eng toggle, sync-caps, page number, vertical table | partial: kb, page size, sel-keys, space-select, auto-learn, full-width, easy-symbol, auto-shift, phrase direction, phrase-from-last, esc-clean, max length, plain (conversion-engine=simple emulated) |
| Candidate window | page numbers, orientation, mouse selection | partial: COSMIC popup, no page number/mouse |
| User dictionary | ibus-chewing SQLite dictionary | done |

## Phases

1. Candidate interaction: cursor, Tab/Shift+Tab, arrows, Enter/Space selection,
   0/custom selection keys, j/k paging, dismissal semantics, tests.
2. Full-width/easy symbols: composing punctuation, Shift+Space, easy symbols,
   Shift+letter parity.
3. Mode keys: Caps Lock / Chi-Eng toggle, sync-caps, editing keys
   (Delete/Home/End), preedit cursor.
4. Phrase behaviour: Shift+Left/Right, add-phrase direction,
   phrase-choice-from-last, auto-shift, plain Zhuyin, max length, esc-clean,
   focus-out.
5. Settings UI and candidate window: expose all settings, page numbers,
   orientation, mouse selection.
6. Automated parity suite: table-driven key sequences; no manual testing.

## Current package

`cosmic-ext-ime 0.2.1+chewing10` replaces the cached-candidate workaround
with libchewing's native select mode, giving true ibus-chewing behaviour:

* The candidate window only opens on demand (Down / Space / backtick symbol
  key); typing continues with the window closed.
* While the window shows, letters are absorbed, Backspace/Esc dismiss the
  window and keep the composition, Enter selects the highlighted candidate,
  Left/Right move the highlight and page at the edges, Up/Down/Space page,
  and Tab/Delete are absorbed no-ops.
* Candidate selection puts the chosen candidate into the preedit; Enter
  commits the buffer (or adds a user phrase after Shift+Left/Right range
  selection).
* Ctrl+Num adds a user phrase (Ctrl+0/1 opens the symbol list); other
  Ctrl/Alt/Meta combos are forwarded.
* English sub-mode with nothing composed forwards printable keys to the
  application (ibus-chewing Github 144).
* Unshifted punctuation types bopomofo (`,` → ㄝ, `.` → ㄡ) like the Da Chen
  layout; Shift+punctuation commits full-width marks.

The ibus-chewing behavioural test suite (`test/ibus-chewing-preedit-test.c`)
is ported to `crates/chewing/tests/ibus_chewing_parity.rs` — 中文, 他不重，
他是我兄弟。, mixed English/Chinese, incomplete bopomofo, the long buffer
sentence, Down/Up/Escape window semantics, Home/Delete/End editing, and
full/half shape.  Selections that depend on upstream's dictionary are looked
up by text, since Ubuntu 24.04's libchewing data orders candidates
differently.
