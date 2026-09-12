//! Settings window for popeinput. It only reads and writes the shared
//! cosmic-config keys; the daemon watches them and applies changes live.

use cosmic::app::{Core, Settings, Task};
use cosmic::cosmic_config::{self, Config};
use cosmic::iced::{Length, Size};
use cosmic::widget::{self, settings};
use cosmic::{executor, Application, Element};
use popeinput_config::{CangjieVersion, CharSet, Mode, PopeinputConfig, APP_ID, CONFIG_VERSION};

const SETTINGS_APP_ID: &str = "io.github.wanleung.popeinput.Settings";

fn main() -> cosmic::iced::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    cosmic::app::run::<App>(Settings::default().size(Size::new(560.0, 720.0)), ())
}

#[derive(Debug, Clone)]
enum Message {
    Mode(usize),
    Version(usize),
    PageSize(u32),
    FullwidthChars(bool),
    CharSet(CharSet, bool),
    FollowLayout(bool),
    CtrlSpace(bool),
    ShiftTap(bool),
    FontSize(u32),
    Reloaded(PopeinputConfig),
}

struct App {
    core: Core,
    handler: Option<Config>,
    cfg: PopeinputConfig,
    mode_labels: Vec<&'static str>,
    version_labels: Vec<&'static str>,
}

impl App {
    fn write(&mut self, apply: impl FnOnce(&mut PopeinputConfig, &Config) -> Result<bool, cosmic_config::Error>) {
        let Some(handler) = self.handler.as_ref() else {
            log::error!("no config store available; change not saved");
            return;
        };
        if let Err(e) = apply(&mut self.cfg, handler) {
            log::error!("saving setting: {e}");
        }
    }
}

impl Application for App {
    type Executor = executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = SETTINGS_APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: ()) -> (Self, Task<Message>) {
        let handler = PopeinputConfig::handler()
            .map_err(|e| log::error!("opening cosmic-config: {e}"))
            .ok();
        let cfg = handler.as_ref().map(PopeinputConfig::load).unwrap_or_default();
        let app = App {
            core,
            handler,
            cfg,
            mode_labels: Mode::ALL.iter().map(|m| m.label()).collect(),
            version_labels: CangjieVersion::ALL.iter().map(|v| v.label()).collect(),
        };
        (app, Task::none())
    }

    fn header_start(&self) -> Vec<Element<'_, Message>> {
        vec![widget::text::title3("popeinput 設定").into()]
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Message> {
        cosmic_config::config_subscription::<_, PopeinputConfig>(0u32, APP_ID.into(), CONFIG_VERSION)
            .map(|update| Message::Reloaded(update.config))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Mode(i) => {
                let mode = Mode::ALL[i.min(Mode::ALL.len() - 1)];
                self.write(|c, h| c.set_mode(h, mode));
            }
            Message::Version(i) => {
                let v = CangjieVersion::ALL[i.min(CangjieVersion::ALL.len() - 1)];
                self.write(|c, h| c.set_cangjie_version(h, v));
            }
            Message::PageSize(n) => self.write(|c, h| c.set_page_size(h, n.clamp(1, 9))),
            Message::FullwidthChars(b) => self.write(|c, h| c.set_fullwidth_chars(h, b)),
            Message::CharSet(set, on) => {
                let mut sets = self.cfg.char_sets.clone();
                sets.retain(|s| *s != set);
                if on {
                    sets.push(set);
                }
                sets.sort_by_key(|s| CharSet::ALL.iter().position(|a| a == s));
                self.write(|c, h| c.set_char_sets(h, sets));
            }
            Message::FollowLayout(b) => self.write(|c, h| c.set_follow_layout(h, b)),
            Message::CtrlSpace(b) => self.write(|c, h| c.set_ctrl_space_toggle(h, b)),
            Message::ShiftTap(b) => self.write(|c, h| c.set_shift_tap_toggle(h, b)),
            Message::FontSize(n) => self.write(|c, h| c.set_popup_font_size(h, n.clamp(8, 48))),
            Message::Reloaded(cfg) => self.cfg = cfg,
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let cfg = &self.cfg;
        let mode_idx = Mode::ALL.iter().position(|m| *m == cfg.mode);
        let version_idx = CangjieVersion::ALL.iter().position(|v| *v == cfg.cangjie_version);

        let input = settings::section()
            .title("輸入法 Input method")
            .add(settings::item(
                "輸入方式 Mode",
                widget::dropdown(&self.mode_labels, mode_idx, Message::Mode),
            ))
            .add(settings::item(
                "倉頡版本 Cangjie version",
                widget::dropdown(&self.version_labels, version_idx, Message::Version),
            ))
            .add(settings::item(
                "每頁候選字數 Candidates per page",
                widget::spin_button(cfg.page_size.to_string(), "candidates per page", cfg.page_size, 1, 1, 9, Message::PageSize),
            ))
            .add(
                settings::item::builder("全形字元 Full-width characters")
                    .description("Space, digits and punctuation commit their full-width forms while idle")
                    .toggler(cfg.fullwidth_chars, Message::FullwidthChars),
            );

        let mut sets = settings::section().title("字元集 Character sets");
        for set in CharSet::ALL {
            sets = sets.add(
                settings::item::builder(set.label())
                    .description(set.description())
                    .toggler(cfg.has_char_set(set), move |on| Message::CharSet(set, on)),
            );
        }

        let switching = settings::section()
            .title("切換中英文 Switching Chinese / English")
            .add(
                settings::item::builder("跟隨鍵盤輸入來源 Follow COSMIC input source")
                    .description(
                        "Chinese input is on while a Chinese layout is active. Add \"Chinese\" under \
                         Settings → Keyboard → Input Sources and switch with Super+Space",
                    )
                    .toggler(cfg.follow_layout, Message::FollowLayout),
            )
            .add(
                settings::item::builder("Ctrl+Space")
                    .description("Toggle Chinese input with Ctrl+Space")
                    .toggler(cfg.ctrl_space_toggle, Message::CtrlSpace),
            )
            .add(
                settings::item::builder("單按 Shift Tap Shift")
                    .description("Toggle by pressing and releasing Shift on its own")
                    .toggler(cfg.shift_tap_toggle, Message::ShiftTap),
            );

        let popup = settings::section().title("候選字視窗 Candidate window").add(settings::item(
            "字體大小 Font size",
            widget::spin_button(cfg.popup_font_size.to_string(), "font size", cfg.popup_font_size, 1, 8, 48, Message::FontSize),
        ));

        let note = widget::text::caption("Changes apply immediately — no restart needed.");

        let content = widget::column::with_capacity(5)
            .spacing(24)
            .padding([0, 24, 24, 24])
            .push(note)
            .push(input)
            .push(sets)
            .push(switching)
            .push(popup);

        widget::scrollable(content).width(Length::Fill).into()
    }
}
