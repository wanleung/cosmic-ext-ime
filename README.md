# cosmic-ext-ime — Input Method for COSMIC

[繁體中文說明](README.zh-HK.md)

Native Wayland input method for the [COSMIC](https://system76.com/cosmic) desktop
(Pop!_OS), made for Hong Kong users: **Cangjie (倉頡)** and **Quick (速成)** via
[libcangjie2](https://cangjians.github.io/projects/libcangjie/), smart Pinyin /
Shuangpin via [libpinyin](https://github.com/libpinyin/libpinyin), or any
[RIME](https://rime.im/) schema via librime (Cangjie, Quick, Jyutping, Pinyin, …).

<p>
  <img src="data/screenshot-cangjie.png" alt="Cangjie candidates" height="360">
  <img src="data/screenshot-quick.png" alt="Quick candidates" height="360">
  <img src="data/screenshot-settings.png" alt="Settings" height="360">
</p>

## Why

`cosmic-comp` only implements `zwp_input_method_v2`. Pop!_OS 24.04 ships IBus 1.5.29
(`zwp_input_method_v1` only) and Fcitx5 5.1.7 (incomplete v2 support), so CJK input is
broken out of the box on COSMIC. cosmic-ext-ime talks the v2 protocol directly and needs no
IBus/Fcitx daemon.

## Install

Pop!_OS 24.04 / Ubuntu 24.04 (COSMIC):

```sh
sudo add-apt-repository ppa:wanleungwong/cosmic-ext-ime
sudo apt install cosmic-ext-ime
```

Or grab the `.deb` from the [releases page](https://github.com/wanleung/cosmic-ext-ime/releases)
and `sudo apt install ./cosmic-ext-ime_*.deb`.

From source:

```sh
sudo apt install just       # or: cargo install just
just install-user           # binaries to ~/.local/bin, launcher entries, autostart
```

`just install` does the same system-wide under /usr/local, `just deb` builds
the Debian package locally, and `just ppa` uploads a signed source package to
the PPA. Use one install method, not several — `just uninstall-user` removes
the per-user copy.

After installing, **log out and back in**. The daemon then autostarts with every
COSMIC session, and the fresh session drops any leftover `GTK_IM_MODULE` from a
previous IBus/Fcitx setup (see *Application support*).

Only one input method should hold the seat, so stop IBus or Fcitx5 and remove
their autostart entries first. Starting a second copy of cosmic-ext-ime is
harmless: it finds the lock in `$XDG_RUNTIME_DIR` and exits.

## Typing

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

Pinyin (libpinyin) works the way mainland users expect — type a whole sentence
and let the language model segment it:

| Key | Action |
| --- | --- |
| `a`–`z` | append pinyin; the popup header shows the segmentation (`ni'hao`) |
| `'` | force a syllable break |
| Space | commit the predicted sentence |
| `1`–`9` | pick one word and keep composing the rest |
| Backspace | undo the last word choice, then delete letters |
| punctuation | commit the sentence, then the full-width mark |

Each commit trains the user dictionary, so frequent phrases rise to the top.

## Settings

Open **Input Method Settings** from the app library (search "input method", "ime" or "倉頡"),
or run `cosmic-ext-ime-settings`. Everything applies immediately — no restart:

- Engine: libcangjie (Cangjie / Quick), libpinyin (智能拼音) or RIME
- libcangjie: mode (倉頡 / 速成), Cangjie 3 / 5, and character sets — Big5, HKSCS,
  all Chinese, Kanji, Hiragana, Katakana, Zhuyin, punctuation, symbols
  (kana are typed as `zj` + romaji, e.g. `zja` → あ)
- libpinyin: 全拼 or a 双拼 scheme (自然码／微软／紫光／智能ABC／拼音加加／小鹤),
  fuzzy pinyin, incomplete pinyin; Simplified output
- RIME: pick any deployed schema, redeploy after editing `*.custom.yaml`, open the
  user data folder
- Candidates per page, full-width characters, popup font size
- How to switch Chinese / English (see below)
- **About** (ⓘ in the header): version, licence, credits and links

Settings are stored through cosmic-config in
`~/.config/cosmic/io.github.wanleung.CosmicExtIme/v1/`, one file per key, so any
tool that writes that store — this app, a future COSMIC Settings page, or
`echo Quick > .../v1/mode` — is picked up live by the daemon.

### Switching between Chinese and English

cosmic-ext-ime follows COSMIC's own input-source switcher: add **Chinese** under
Settings → Keyboard → Input Sources, then Super+Space toggles Chinese input on and
off and the panel's input-source applet shows which is active. With only one layout
configured, Chinese input is always on. Ctrl+Space and lone-Shift-tap toggles can
be enabled in the settings app.

### RIME

Install schemas with apt (`rime-data-cangjie5`, `rime-data-quick5`,
`librime-data-jyutping`, …). On first start cosmic-ext-ime writes
`~/.local/share/cosmic-ext-ime/rime/default.custom.yaml` listing every installed
schema so all of them get deployed; edit it to trim the list or tweak RIME the
usual way (`*.custom.yaml`), then press **Redeploy** in the settings app.
User dictionaries live in the same directory ("Open" button).

### Application support

| Toolkit | Works? | Notes |
| --- | --- | --- |
| COSMIC apps (iced) | yes | native `text-input-v3` |
| GTK 3 / GTK 4 (gedit, Nautilus, Firefox, GNOME apps) | yes | native `text-input-v3`; `GTK_IM_MODULE` must be **unset** |
| Chrome / Electron / VS Code | yes | run on Wayland with `--enable-wayland-ime` (e.g. in `~/.config/code-flags.conf`) |
| Qt 6.5+ (Flatpak Qt apps) | yes | native `text-input-v3` |
| Qt 5 / Qt 6.4 (Pop!_OS 24.04 packages) | no | those Qt versions lack `text-input-v3`, the only version cosmic-comp offers |
| X11 / XWayland apps | no | would need an XIM server |

If you previously used IBus or Fcitx, remove `GTK_IM_MODULE`, `QT_IM_MODULE`
and `XMODIFIERS` from `~/.profile` / `/etc/environment` and run
`im-config -n none`, then log in again — with those set, toolkits load the old
IM module instead of talking to the compositor. cosmic-ext-ime logs a warning at
start-up when it sees them.

## Troubleshooting

- Logs: `journalctl --user -b | grep cosmic-ext-ime` (the daemon is started by
  the session through systemd; it restarts automatically if it crashes).
- Typing works in COSMIC apps but not GTK apps: `GTK_IM_MODULE` is still set in
  the session; see *Application support* above and log in again. Apps launched
  from the panel keep the old environment until you log out.
- Nothing happens at all: another input method may hold the seat. Check for
  IBus/Fcitx5 (`pgrep -a ibus-daemon fcitx5`) and stop it.
- "another cosmic-ext-ime is already running": the session already autostarted
  one; use that instead of launching a second by hand.
- RIME schema missing from the list: add it to
  `~/.local/share/cosmic-ext-ime/rime/default.custom.yaml` and press Redeploy.

## Layout

```
src/                  daemon: Wayland frontend (zwp_input_method_v2 + zwp_virtual_keyboard_v1)
settings/             cosmic-ext-ime-settings, a libcosmic app writing the shared config
crates/config         settings schema stored via cosmic-config (the daemon/UI contract)
crates/engine         InputEngine trait + shared types (no Wayland, no FFI)
crates/cangjie-sys    bindgen FFI to libcangjie2
crates/cangjie        safe wrapper + Cangjie/Quick engine (tested against the real DB)
crates/rime-sys       bindgen FFI to librime's C API
crates/rime           RIME engine: one librime session per engine, schemas from rime-data
crates/pinyin-sys     bindgen FFI to libpinyin
crates/pinyin         smart Pinyin engine (sentence prediction, Shuangpin, fuzzy pinyin)
```

## Development

```sh
sudo apt install libcangjie2-dev libsqlite3-dev librime-dev libpinyin15-dev libpinyin-data libglib2.0-dev libxkbcommon-dev libclang-dev libwayland-dev
cargo test --workspace     # engine tests hit the real libcangjie2, rime-data and libpinyin data
RUST_LOG=debug cargo run --release -p cosmic-ext-ime
```

Releasing: `just release X.Y.Z "summary"` bumps the version in `Cargo.toml`,
`debian/changelog`, the AppStream metainfo and the man pages, then commits and
tags. Push the tag (CI builds the GitHub release) and run `just ppa`.

`libcosmic` (used only by the settings app) is pinned to a revision whose MSRV
still matches the newest Rust toolchain available on Launchpad for Ubuntu 24.04;
check `cargo +1.91.1 build` before bumping it.

## Roadmap

- [x] Candidate popup via `zwp_input_popup_surface_v2` (cosmic-text + wl_shm)
- [x] Settings app (libcosmic) with live reload via cosmic-config
- [x] Autostart via XDG autostart entry
- [x] HiDPI / fractional scaling for the popup; follows the COSMIC theme
- [x] librime engine
- [x] libpinyin engine (smart Pinyin / Shuangpin)
- [x] Debian packaging (`just deb`) and a PPA
- [ ] Traditional Chinese output for libpinyin (OpenCC)
- [ ] Integration in COSMIC Settings → Keyboard (upstream)

## License

GPL-3.0-or-later (see `LICENSE`). The FFI binding crates follow the libraries
they wrap: `crates/cangjie-sys` is LGPL-3.0-or-later, `crates/rime-sys` is
BSD-3-Clause and `crates/pinyin-sys` is GPL-2.0-or-later.
