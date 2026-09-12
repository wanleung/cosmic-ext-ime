# popeinput

Native Wayland Chinese input method for the [COSMIC](https://system76.com/cosmic) desktop
(Pop!_OS), built for Hong Kong users: **Cangjie (倉頡)** and **Quick (速成)** via
[libcangjie2](https://cangjians.github.io/projects/libcangjie/), or any
[RIME](https://rime.im/) schema via librime (Cangjie, Quick, Jyutping, Pinyin, …).

## Why

`cosmic-comp` only implements `zwp_input_method_v2`. Pop!_OS 24.04 ships IBus 1.5.29
(`zwp_input_method_v1` only) and Fcitx5 5.1.7 (incomplete v2 support), so CJK input is
broken out of the box on COSMIC. popeinput talks the v2 protocol directly and needs no
IBus/Fcitx daemon.

Key handling deliberately mirrors [ibus-cangjie](https://github.com/wanleung/ibus-cangjie):

| Key | Cangjie | Quick |
| --- | --- | --- |
| `a`–`z` | append radical (max 5) | append radical (max 2), auto-lookup at 2 |
| `*` | wildcard (after first key) | full-width `＊` |
| Space | look up; commit if unique; next page if >1 page | same |
| `1`–`9` | pick candidate | pick candidate |
| new key with candidates shown | commits first candidate, then starts new input | same |
| Backspace / Escape | drop last key / cancel | same |
| Page Up/Down, ↑/↓ | page (wraps) | same |
| punctuation, digits, Space (idle) | full-width from libcangjie's shortcode table | same |
| Super+Space (COSMIC Input Sources) | Chinese on while a Chinese layout is active | same |

## Layout

```
src/                  daemon: Wayland frontend (zwp_input_method_v2 + zwp_virtual_keyboard_v1)
settings/             popeinput-settings, a libcosmic app writing the shared config
crates/config         settings schema stored via cosmic-config (the daemon/UI contract)
crates/engine         InputEngine trait + shared types (no Wayland, no FFI)
crates/cangjie-sys    bindgen FFI to libcangjie2
crates/cangjie        safe wrapper + Cangjie/Quick engine (tested against the real DB)
crates/rime-sys       bindgen FFI to librime's C API
crates/rime           RIME engine: one librime session per engine, schemas from rime-data
```

## Install

```sh
cargo install just          # or: sudo apt install just
just install-user           # binaries to ~/.local/bin, launcher entries, autostart
```

`just install` does the same system-wide under /usr/local. After installing,
log out and back in (or run `popeinput` once by hand) — it autostarts with every
COSMIC session from then on.

Only one input method may hold the seat: stop IBus/Fcitx5 first (`ibus exit`,
`fcitx5-remote -e`), otherwise popeinput logs "input method unavailable" and exits.

## Settings

Open **popeinput Settings** from the app library (search "popeinput" or "倉頡"),
or run `popeinput-settings`. Everything applies immediately — no restart:

- Engine: libcangjie (Cangjie / Quick) or RIME
- RIME: pick any deployed schema; the list is published by the daemon once RIME starts
- libcangjie: Mode (倉頡 / 速成), Cangjie 3 / 5, candidates per page, full-width characters
- Character sets: Big5, HKSCS, all Chinese, Kanji, Hiragana, Katakana, Zhuyin,
  punctuation, symbols (kana are typed as `zj` + romaji, e.g. `zja` → あ)
- How to switch Chinese / English (see below)
- Candidate window font size

Settings are stored through cosmic-config in
`~/.config/cosmic/io.github.wanleung.popeinput/v1/`, one file per key, so any
tool that writes that store — this app, a future COSMIC Settings page, or
`echo Quick > .../v1/mode` — is picked up live by the daemon.

### RIME

Install schemas with apt (`rime-data-cangjie5`, `rime-data-quick5`,
`librime-data-jyutping`, …). On first start popeinput writes
`~/.local/share/popeinput/rime/default.custom.yaml` listing every installed
schema so all of them get deployed; edit it to trim the list or tweak RIME the
usual way (`*.custom.yaml`). User dictionaries live in the same directory.

### Switching between Chinese and English

popeinput follows COSMIC's own input-source switcher: add **Chinese** under
Settings → Keyboard → Input Sources, then Super+Space toggles Cangjie on/off and
the panel's input-source applet shows which is active. With only one layout
configured, Chinese input is always on. Ctrl+Space and lone-Shift-tap toggles can
be enabled in the settings app.

## Development

```sh
sudo apt install libcangjie2-dev libsqlite3-dev librime-dev libxkbcommon-dev libclang-dev libwayland-dev
cargo test --workspace     # engine tests hit the real libcangjie2 and rime-data
RUST_LOG=debug cargo run --release -p popeinput
```

`libcosmic` (used only by the settings app) is pinned to the revision that the
installed `cosmic-settings` is built from; bump it in `Cargo.toml` when COSMIC updates.

## Roadmap

- [x] Candidate popup via `zwp_input_popup_surface_v2` (cosmic-text + wl_shm)
- [x] Settings app (libcosmic) with live reload via cosmic-config
- [x] Autostart via XDG autostart entry
- [ ] HiDPI / fractional scaling for the popup; follow the COSMIC theme
- [x] librime engine
- [ ] Pass key releases to RIME (needed for its Shift-toggles-ASCII option)
- [ ] Integration in COSMIC Settings → Keyboard (upstream)
- [ ] Debian packaging for Pop!_OS
