# popeinput

Native Wayland Chinese input method for the [COSMIC](https://system76.com/cosmic) desktop
(Pop!_OS), built for Hong Kong users: **Cangjie (倉頡)** and **Quick (速成)** via
[libcangjie2](https://cangjians.github.io/projects/libcangjie/), with
[RIME](https://rime.im/) planned as a second engine.

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
src/                  Wayland frontend (zwp_input_method_v2 + zwp_virtual_keyboard_v1)
crates/engine         InputEngine trait + shared types (no Wayland, no FFI)
crates/cangjie-sys    bindgen FFI to libcangjie2
crates/cangjie        safe wrapper + Cangjie/Quick engine (tested against the real DB)
```

## Build

```sh
sudo apt install libcangjie2-dev libsqlite3-dev libxkbcommon-dev libclang-dev libwayland-dev
cargo build --release
cargo test --workspace     # engine tests hit the real libcangjie2 database
```

## Run (inside a COSMIC session)

```sh
./target/release/popeinput            # Cangjie
./target/release/popeinput quick      # Quick / 速成
RUST_LOG=debug ./target/release/popeinput
```

Only one input method may hold the seat: stop IBus/Fcitx5 first (`ibus exit`, `fcitx5-remote -e`),
otherwise popeinput logs "input method unavailable" and exits.

### Switching between Chinese and English

popeinput follows COSMIC's own input-source switcher: add **Chinese** under
Settings → Keyboard → Input Sources, then Super+Space toggles Cangjie on/off and
the panel's input-source applet shows which is active. With only one layout
configured, Chinese input is always on.

### Configuration

Optional, at `~/.config/popeinput/config.toml` (all keys optional):

```toml
mode = "cangjie"          # or "quick"
version = 5               # Cangjie 3 or 5
filters = ["big5", "hkscs"]
page_size = 9
fullwidth_chars = true    # full-width digits/punctuation/space while idle

[toggle]
follow_layout = true
chinese_layouts = ["chinese", "cantonese", "hong kong", "taiwanese"]
ctrl_space = false        # Ctrl+Space toggle
shift_tap = false         # lone Shift tap toggle

[popup]
font_size = 18.0
background = "#1e1e1e"
foreground = "#f0f0f0"
hint = "#8a8a8a"
border = "#5a5a5a"
```

## Roadmap

- [x] Candidate popup via `zwp_input_popup_surface_v2` (cosmic-text + wl_shm)
- [x] Config file
- [ ] HiDPI / fractional scaling for the popup; follow the COSMIC theme
- [ ] librime engine (`crates/rime-sys`, `crates/rime`) — Jyutping, rime-cangjie schemas
- [ ] Autostart (systemd user unit / cosmic-session)
- [ ] Debian packaging for Pop!_OS
