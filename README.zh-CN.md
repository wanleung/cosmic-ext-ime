# cosmic-ext-ime — COSMIC 输入法

[English](README.md) ・ [繁體中文（香港）](README.zh-HK.md) ・ [繁體中文（台灣）](README.zh-TW.md)

为 [COSMIC](https://system76.com/cosmic) 桌面（Pop!_OS）编写的原生 Wayland 输入法：
用 [libpinyin](https://github.com/libpinyin/libpinyin) 输入**智能拼音／双拼**，
用 [libcangjie2](https://cangjians.github.io/projects/libcangjie/) 输入**仓颉**和**速成**，
也可以通过 librime 使用任何 [RIME](https://rime.im/) 方案（拼音、双拼、五笔、粤拼等）。

<p>
  <img src="data/screenshot-cangjie.png" alt="仓颉候选字" height="360">
  <img src="data/screenshot-quick.png" alt="速成候选字" height="360">
  <img src="data/screenshot-settings.png" alt="设置" height="360">
</p>

## 背景

`cosmic-comp` 只实现了 `zwp_input_method_v2`。Pop!_OS 24.04 自带的 IBus 1.5.29
只支持 `zwp_input_method_v1`，Fcitx5 5.1.7 对 v2 的支持也不完整，所以在 COSMIC
上中日韩输入法开箱即坏。cosmic-ext-ime 直接与合成器通信，不需要 IBus 或 Fcitx 守护进程。

## 安装

Pop!_OS 24.04 / Ubuntu 24.04（COSMIC）：

```sh
sudo add-apt-repository ppa:wanleungwong/cosmic-ext-ime
sudo apt install cosmic-ext-ime
```

也可以到[发布页面](https://github.com/wanleung/cosmic-ext-ime/releases)下载 `.deb`，
然后执行 `sudo apt install ./cosmic-ext-ime_*.deb`。

从源码安装：

```sh
sudo apt install just       # 或者：cargo install just
just install-user           # 安装到 ~/.local/bin，含启动器条目和自启动
```

`just install` 是对应的系统级安装（/usr/local），`just deb` 在本机打包，
`just ppa` 上传已签名的源码包到 PPA。请只选用一种安装方式；
`just uninstall-user` 可删除用户目录下的副本。

安装后请**注销并重新登录**。之后输入法会随每个 COSMIC 会话自动启动，
新会话也会清掉旧 IBus／Fcitx 配置留下的 `GTK_IM_MODULE`（见〈程序支持〉）。

同一时间只应有一个输入法占用 seat，所以请先停用 IBus 或 Fcitx5 并删除它们的自启动项。
重复启动 cosmic-ext-ime 没有危害：它会发现 `$XDG_RUNTIME_DIR` 中的锁并自行退出。

## 输入方式

拼音（libpinyin）就是熟悉的整句输入，由语言模型自动分词：

| 按键 | 作用 |
| --- | --- |
| `a`–`z` | 输入拼音；候选窗顶部显示分词结果（`ni'hao`） |
| `'` | 强制分隔音节 |
| Space | 上屏预测出来的整句 |
| `1`–`9` | 选择其中一个词，继续输入剩余部分 |
| Backspace | 先撤销上一次选词，然后删除字母 |
| 标点 | 先上屏句子，再输入全角标点 |

每次上屏都会训练用户词库，常用词会逐渐靠前。支持模糊音（z/zh、c/ch、s/sh、n/l、
an/ang、en/eng、in/ing 等）和简拼（例如 `nh` → 你好），输出为简体字。

仓颉／速成的按键行为与 [ibus-cangjie](https://github.com/wanleung/ibus-cangjie) 保持一致：

| 按键 | 仓颉 | 速成 |
| --- | --- | --- |
| `a`–`z` | 输入字根（最多 5 码） | 输入字根（最多 2 码），满 2 码即时查字 |
| `*` | 通配符（首码之后） | 全角 `＊` |
| Space | 查字；只有一个直接上屏；多于一页则翻页 | 同上 |
| `1`–`9` | 选字 | 选字 |
| 有候选字时再按字根 | 先上屏第一个候选字，再开始新的输入 | 同上 |
| Backspace／Escape | 删除上一码／取消 | 同上 |
| Page Up/Down、↑／↓ | 翻页（循环） | 同上 |
| 标点、数字、Space（无输入时） | 取 libcangjie shortcode 表中的全角字符 | 同上 |

## 设置

在应用列表中搜索「输入法」「input method」或「仓颉」打开
**输入法设置 Input Method Settings**，或执行 `cosmic-ext-ime-settings`。
所有设置立即生效，无需重启：

- 输入引擎：libpinyin（智能拼音）、libcangjie（仓颉／速成）或 RIME
- libpinyin：全拼或双拼方案（自然码／微软／紫光／智能ABC／拼音加加／小鹤）、
  模糊音、简拼；输出简体字
- libcangjie：输入方式（仓颉／速成）、仓颉三代／五代，以及字符集 —
  Big5、香港增补字符集 HKSCS、全部汉字、日文汉字、平假名、片假名、注音、标点、符号
  （假名用 `zj` 加罗马字，例如 `zja` → あ）
- RIME：选择任意已部署的方案，修改 `*.custom.yaml` 后重新部署，打开用户数据文件夹
- 每页候选字数、全角字符、候选窗字号
- 中英文切换方式（见下文）
- **关于**（标题栏的 ⓘ）：版本、许可证、鸣谢和链接

设置通过 cosmic-config 保存在
`~/.config/cosmic/io.github.wanleung.CosmicExtIme/v1/`，每项一个文件。
任何写入该位置的工具 — 设置程序、将来 COSMIC 设置中的页面，
甚至 `echo Quick > .../v1/mode` — 都会被输入法实时读取。

### 中英文切换

cosmic-ext-ime 跟随 COSMIC 自身的输入源：在
设置 → Keyboard → Input Sources 中添加 **Chinese**，之后 Super+Space
即可开关中文输入，面板上的输入源指示器会显示当前状态。
如果只配置了一种键盘布局，中文输入将始终开启。
设置程序中也可以启用 Ctrl+Space 或轻按 Shift 切换。

### RIME

用 apt 安装方案，例如：

```sh
sudo apt install rime-data-luna-pinyin rime-data-double-pinyin rime-data-wubi rime-data-pinyin-simp
```

首次启动时 cosmic-ext-ime 会写入
`~/.local/share/cosmic-ext-ime/rime/default.custom.yaml`，列出所有已安装方案以便全部部署；
可以自行精简列表，或按 RIME 常规做法用 `*.custom.yaml` 定制，
改完后在设置程序中点**重新部署 Redeploy**。用户词典也在同一文件夹（点「打开」即可）。

简体用户常用的方案：`luna_pinyin_simp`（朙月拼音·简化字）、
`double_pinyin*`（双拼）、`wubi86`（五笔）。

### 程序支持

| 工具包 | 可用？ | 说明 |
| --- | --- | --- |
| COSMIC 程序（iced） | 可以 | 原生 `text-input-v3` |
| GTK 3／GTK 4（gedit、Nautilus、Firefox、GNOME 程序） | 可以 | 原生 `text-input-v3`；`GTK_IM_MODULE` 必须**未设置** |
| Chrome／Electron／VS Code | 可以 | 以 Wayland 模式运行并加上 `--enable-wayland-ime`（例如写入 `~/.config/code-flags.conf`） |
| Qt 6.5+（Flatpak 版 Qt 程序） | 可以 | 原生 `text-input-v3` |
| Qt 5／Qt 6.4（Pop!_OS 24.04 软件包） | 不可以 | 这些 Qt 版本不支持 `text-input-v3`，而 cosmic-comp 只提供这一版本 |
| X11／XWayland 程序 | 不可以 | 需要 XIM 服务器 |

如果以前用过 IBus 或 Fcitx，请在 `~/.profile`／`/etc/environment` 中删除
`GTK_IM_MODULE`、`QT_IM_MODULE` 和 `XMODIFIERS`，再执行 `im-config -n none`，
然后重新登录。这些变量存在时，工具包会加载旧的 IM 模块，而不与合成器通信。
cosmic-ext-ime 启动时检测到它们会给出警告。

## 疑难解答

- 日志：`journalctl --user -b | grep cosmic-ext-ime`
  （输入法由会话经 systemd 启动，崩溃后会自动重启）。
- COSMIC 程序能输入、GTK 程序不能：会话中仍设置了 `GTK_IM_MODULE`，
  请参考〈程序支持〉并重新登录。从面板启动的程序在注销前会沿用旧环境。
- 完全没有反应：可能有其他输入法占用 seat。用 `pgrep -a ibus-daemon fcitx5` 检查并停用。
- 出现「another cosmic-ext-ime is already running」：会话已经自动启动了一个，
  直接使用即可，不必再手动启动。
- RIME 方案列表里找不到某个方案：在
  `~/.local/share/cosmic-ext-ime/rime/default.custom.yaml` 中添加，再点重新部署。

## 项目结构

```
src/                  输入法本体：Wayland 前端（zwp_input_method_v2 + zwp_virtual_keyboard_v1）
settings/             cosmic-ext-ime-settings，用 libcosmic 编写的设置程序
crates/config         设置的数据结构，经 cosmic-config 保存（本体与界面之间的契约）
crates/engine         InputEngine trait 及共用类型（不含 Wayland、不含 FFI）
crates/cangjie-sys    libcangjie2 的 bindgen FFI
crates/cangjie        安全封装及仓颉／速成引擎（用真实数据库测试）
crates/rime-sys       librime C API 的 bindgen FFI
crates/rime           RIME 引擎：每个引擎一个 librime 会话，方案来自 rime-data
crates/pinyin-sys     libpinyin 的 bindgen FFI
crates/pinyin         智能拼音引擎（整句预测、双拼、模糊音）
```

## 开发

```sh
sudo apt install libcangjie2-dev libsqlite3-dev librime-dev libpinyin15-dev libpinyin-data libglib2.0-dev libxkbcommon-dev libclang-dev libwayland-dev
cargo test --workspace     # 测试会用真实的 libcangjie2、rime-data 和 libpinyin 数据
RUST_LOG=debug cargo run --release -p cosmic-ext-ime
```

发布：`just release X.Y.Z "摘要"` 会更新 `Cargo.toml`、`debian/changelog`、
AppStream metainfo 和 man page 中的版本号，然后提交并打 tag。
推送 tag（CI 会生成 GitHub release），再执行 `just ppa`。

`libcosmic`（仅设置程序使用）锁定在 MSRV 仍然匹配 Launchpad 上 Ubuntu 24.04
最新 Rust 工具链的版本；升级前请先用 `cargo +1.91.1 build` 验证。

## 路线图

- [x] 用 `zwp_input_popup_surface_v2` 显示候选窗（cosmic-text + wl_shm）
- [x] 设置程序（libcosmic），经 cosmic-config 实时生效
- [x] 通过 XDG autostart 自启动
- [x] 候选窗支持 HiDPI／小数缩放，并跟随 COSMIC 主题
- [x] librime 引擎
- [x] libpinyin 引擎（智能拼音／双拼）
- [x] Debian 打包（`just deb`）及 PPA
- [ ] libpinyin 输出繁体字（OpenCC）
- [ ] 集成到 COSMIC 设置 → Keyboard（上游）

## 许可证

GPL-3.0-or-later（见 `LICENSE`）。FFI 绑定 crate 遵循各自封装的库：
`crates/cangjie-sys` 为 LGPL-3.0-or-later，`crates/rime-sys` 为 BSD-3-Clause，
`crates/pinyin-sys` 为 GPL-2.0-or-later。
