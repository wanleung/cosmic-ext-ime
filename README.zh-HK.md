# cosmic-ext-ime — COSMIC 輸入法

[English](README.md)

為 [COSMIC](https://system76.com/cosmic) 桌面（Pop!_OS）而設的原生 Wayland 輸入法，
以香港用家為本：用 [libcangjie2](https://cangjians.github.io/projects/libcangjie/)
輸入**倉頡**同**速成**，用 [libpinyin](https://github.com/libpinyin/libpinyin)
輸入智能拼音／雙拼，亦可以透過 librime 使用任何
[RIME](https://rime.im/) 方案（倉頡、速成、粵拼、拼音等）。

<p>
  <img src="data/screenshot-cangjie.png" alt="倉頡候選字" height="360">
  <img src="data/screenshot-quick.png" alt="速成候選字" height="360">
  <img src="data/screenshot-settings.png" alt="設定" height="360">
</p>

## 緣由

`cosmic-comp` 只實作了 `zwp_input_method_v2`。Pop!_OS 24.04 附帶的 IBus 1.5.29
只支援 `zwp_input_method_v1`，Fcitx5 5.1.7 對 v2 的支援亦未完整，所以 COSMIC
一開始就打唔到中文。cosmic-ext-ime 直接同 compositor 講 v2 協定，不需要 IBus
或 Fcitx 常駐程式。

## 安裝

Pop!_OS 24.04 / Ubuntu 24.04（COSMIC）：

```sh
sudo add-apt-repository ppa:wanleungwong/cosmic-ext-ime
sudo apt install cosmic-ext-ime
```

亦可以在[發布頁面](https://github.com/wanleung/cosmic-ext-ime/releases)下載 `.deb`，
再執行 `sudo apt install ./cosmic-ext-ime_*.deb`。

由原始碼安裝：

```sh
sudo apt install just       # 或者：cargo install just
just install-user           # 安裝到 ~/.local/bin，連同啟動器項目及自動啟動
```

`just install` 是系統層面（/usr/local）的相同安裝，`just deb` 在本機打包，
`just ppa` 上載已簽署的原始碼套件到 PPA。只用其中一種安裝方式；
`just uninstall-user` 可移除個人目錄的副本。

安裝後請**登出再登入**。之後輸入法會隨每個 COSMIC session 自動啟動，
而新 session 亦會清走舊 IBus／Fcitx 設定留低的 `GTK_IM_MODULE`（見〈程式支援〉）。

同一時間只應有一個輸入法佔用 seat，所以請先停用 IBus 或 Fcitx5 並移除其自動啟動。
重複開啟 cosmic-ext-ime 並無害：它會發現 `$XDG_RUNTIME_DIR` 內的 lock 並自行結束。

## 輸入方式

按鍵行為刻意與 [ibus-cangjie](https://github.com/wanleung/ibus-cangjie) 一致：

| 按鍵 | 倉頡 | 速成 |
| --- | --- | --- |
| `a`–`z` | 加入字根（最多 5 個） | 加入字根（最多 2 個），夠兩碼即時查字 |
| `*` | 萬用字元（第一碼之後） | 全形 `＊` |
| Space | 查字；只有一個就直接上屏；多於一頁就揭下一頁 | 同上 |
| `1`–`9` | 選字 | 選字 |
| 有候選字時再按字根 | 先上屏第一個候選字，再開始新的輸入 | 同上 |
| Backspace／Escape | 刪除上一碼／取消 | 同上 |
| Page Up/Down、↑／↓ | 揭頁（循環） | 同上 |
| 標點、數字、Space（無輸入時） | 由 libcangjie 的 shortcode 表取全形字元 | 同上 |

拼音（libpinyin）則照大陸用家的習慣，可以一次過打成句，再由語言模型斷詞：

| 按鍵 | 動作 |
| --- | --- |
| `a`–`z` | 加入拼音；候選字視窗頂部顯示斷詞（`ni'hao`） |
| `'` | 強制分隔音節 |
| Space | 上屏預測出來的句子 |
| `1`–`9` | 選其中一個詞，繼續輸入其餘部分 |
| Backspace | 先還原上一次選詞，再刪除字母 |
| 標點 | 先上屏句子，再輸入全形標點 |

每次上屏都會訓練使用者詞典，常用詞會逐漸排前。

## 設定

在 App Library 搜尋「輸入法」、「input method」或「倉頡」開啟
**輸入法設定 Input Method Settings**，或執行 `cosmic-ext-ime-settings`。
所有設定即時生效，不用重新啟動：

- 輸入引擎：libcangjie（倉頡／速成）、libpinyin（智能拼音）或 RIME
- libcangjie：輸入方式（倉頡／速成）、倉頡三代／五代，以及字元集 —
  Big5、香港增補字符集 HKSCS、所有中文字、日文漢字、平假名、片假名、注音、標點、符號
  （假名用 `zj` 加羅馬拼音，例如 `zja` → あ）
- libpinyin：全拼或雙拼方案（自然碼／微軟／紫光／智能ABC／拼音加加／小鶴）、
  模糊音、簡拼；輸出為簡體字
- RIME：選擇任何已部署的方案、改完 `*.custom.yaml` 後重新部署、開啟使用者資料夾
- 每頁候選字數、全形字元、候選字視窗字體大小
- 中英文切換方式（見下文）
- **關於**（標題列的 ⓘ）：版本、授權、鳴謝及連結

設定透過 cosmic-config 儲存在
`~/.config/cosmic/io.github.wanleung.CosmicExtIme/v1/`，每個設定一個檔案。
任何寫入這個位置的工具 — 設定程式、將來 COSMIC Settings 內的頁面，
甚至 `echo Quick > .../v1/mode` — 都會被輸入法即時讀取。

### 中英文切換

cosmic-ext-ime 跟隨 COSMIC 本身的輸入來源：在
設定 → Keyboard → Input Sources 加入 **Chinese**，之後 Super+Space
就可以開關中文輸入，面板上的輸入來源指示亦會顯示現時狀態。
如果只設定了一種鍵盤 layout，中文輸入會一直開啟。
設定程式內亦可啟用 Ctrl+Space 或單按 Shift 切換。

### RIME

用 apt 安裝方案（`rime-data-cangjie5`、`rime-data-quick5`、
`librime-data-jyutping` 等）。首次啟動時 cosmic-ext-ime 會寫入
`~/.local/share/cosmic-ext-ime/rime/default.custom.yaml`，列出所有已安裝方案以便全部部署；
可以自行編輯精簡清單，或按 RIME 慣常做法用 `*.custom.yaml` 自訂，
改完在設定程式按**重新部署 Redeploy**。使用者詞典亦放在同一個資料夾（按「開啟」即可）。

### 程式支援

| 工具套件 | 可用？ | 備註 |
| --- | --- | --- |
| COSMIC 程式（iced） | 可以 | 原生 `text-input-v3` |
| GTK 3／GTK 4（gedit、Nautilus、Firefox、GNOME 程式） | 可以 | 原生 `text-input-v3`；`GTK_IM_MODULE` 必須**未設定** |
| Chrome／Electron／VS Code | 可以 | 以 Wayland 模式執行並加上 `--enable-wayland-ime`（例如寫入 `~/.config/code-flags.conf`） |
| Qt 6.5+（Flatpak 版 Qt 程式） | 可以 | 原生 `text-input-v3` |
| Qt 5／Qt 6.4（Pop!_OS 24.04 套件） | 不可以 | 這些 Qt 版本未支援 `text-input-v3`，而 cosmic-comp 只提供這個版本 |
| X11／XWayland 程式 | 不可以 | 需要 XIM server |

如果以前用過 IBus 或 Fcitx，請在 `~/.profile`／`/etc/environment` 移除
`GTK_IM_MODULE`、`QT_IM_MODULE` 及 `XMODIFIERS`，再執行 `im-config -n none`，
然後重新登入。這些變數存在時，工具套件會載入舊的 IM module，
而不會同 compositor 溝通。cosmic-ext-ime 啟動時見到它們會發出警告。

## 疑難排解

- 記錄：`journalctl --user -b | grep cosmic-ext-ime`
  （輸入法由 session 經 systemd 啟動，崩潰後會自動重啟）。
- COSMIC 程式打到字，GTK 程式打唔到：session 內仍然設定了 `GTK_IM_MODULE`，
  請參考〈程式支援〉並重新登入。由面板開啟的程式會沿用舊環境，直至登出為止。
- 完全無反應：可能有其他輸入法佔用 seat。用 `pgrep -a ibus-daemon fcitx5` 檢查並停用。
- 出現「another cosmic-ext-ime is already running」：session 已經自動啟動了一個，
  直接用它就可以，不需要再手動開啟。
- RIME 方案清單見不到某個方案：在
  `~/.local/share/cosmic-ext-ime/rime/default.custom.yaml` 加入，再按重新部署。

## 專案結構

```
src/                  輸入法本體：Wayland 前端（zwp_input_method_v2 + zwp_virtual_keyboard_v1）
settings/             cosmic-ext-ime-settings，用 libcosmic 寫的設定程式
crates/config         設定的資料結構，經 cosmic-config 儲存（本體與 UI 之間的介面）
crates/engine         InputEngine trait 及共用型別（不含 Wayland、不含 FFI）
crates/cangjie-sys    libcangjie2 的 bindgen FFI
crates/cangjie        安全封裝及倉頡／速成引擎（用真實資料庫測試）
crates/rime-sys       librime C API 的 bindgen FFI
crates/rime           RIME 引擎：每個引擎一個 librime session，方案來自 rime-data
crates/pinyin-sys     libpinyin 的 bindgen FFI
crates/pinyin         智能拼音引擎（整句預測、雙拼、模糊音）
```

## 開發

```sh
sudo apt install libcangjie2-dev libsqlite3-dev librime-dev libpinyin15-dev libpinyin-data libglib2.0-dev libxkbcommon-dev libclang-dev libwayland-dev
cargo test --workspace     # 測試會用真實的 libcangjie2、rime-data 及 libpinyin 資料
RUST_LOG=debug cargo run --release -p cosmic-ext-ime
```

發布：`just release X.Y.Z "摘要"` 會更新 `Cargo.toml`、`debian/changelog`、
AppStream metainfo 及 man page 的版本號，然後 commit 及打 tag。
推送 tag（CI 會產生 GitHub release），再執行 `just ppa`。

`libcosmic`（只有設定程式使用）鎖定在一個 MSRV 仍然配合 Launchpad
上 Ubuntu 24.04 最新 Rust 工具鏈的版本；升級前請先用
`cargo +1.91.1 build` 驗證。

## 發展路線

- [x] 用 `zwp_input_popup_surface_v2` 顯示候選字視窗（cosmic-text + wl_shm）
- [x] 設定程式（libcosmic），經 cosmic-config 即時生效
- [x] 以 XDG autostart 自動啟動
- [x] 候選字視窗支援 HiDPI／小數縮放，並跟隨 COSMIC 主題
- [x] librime 引擎
- [x] libpinyin 引擎（智能拼音／雙拼）
- [x] Debian 打包（`just deb`）及 PPA
- [ ] libpinyin 輸出繁體字（OpenCC）
- [ ] 整合到 COSMIC Settings → Keyboard（上游）

## 授權

GPL-3.0-or-later（見 `LICENSE`）。FFI 綁定的 crate 跟隨其封裝的函式庫：
`crates/cangjie-sys` 為 LGPL-3.0-or-later，`crates/rime-sys` 為 BSD-3-Clause，
`crates/pinyin-sys` 為 GPL-2.0-or-later。
