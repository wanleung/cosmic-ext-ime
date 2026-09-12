mod config;
mod im;
mod keys;
mod popup;

use anyhow::{Context, Result};
use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use cosmic_config::calloop::ConfigWatchSource;
use cosmic_config::CosmicConfigEntry;
use cosmic_ext_ime_config::PopeinputConfig;
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::{wl_compositor::WlCompositor, wl_seat::WlSeat, wl_shm::WlShm};
use wayland_client::Connection;
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;
use wayland_protocols::wp::viewporter::client::wp_viewporter::WpViewporter;
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_manager_v2::ZwpInputMethodManagerV2;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;

use crate::im::State;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    warn_about_im_modules();

    let handler = PopeinputConfig::handler().context("opening cosmic-config store")?;
    let cfg = PopeinputConfig::load(&handler);
    log::debug!("config: {cfg:?}");
    let engine = config::build_engine(&cfg)?;

    let conn = Connection::connect_to_env().context("connecting to Wayland display")?;
    let (globals, queue) = registry_queue_init::<State>(&conn)?;
    let qh = queue.handle();

    let seat: WlSeat = globals.bind(&qh, 1..=7, ()).context("wl_seat")?;
    let compositor: WlCompositor = globals.bind(&qh, 4..=6, ()).context("wl_compositor")?;
    let shm: WlShm = globals.bind(&qh, 1..=1, ()).context("wl_shm")?;
    let fractional_scale: Option<WpFractionalScaleManagerV1> = globals.bind(&qh, 1..=1, ()).ok();
    let viewporter: Option<WpViewporter> = globals.bind(&qh, 1..=1, ()).ok();
    let im_manager: ZwpInputMethodManagerV2 = globals
        .bind(&qh, 1..=1, ())
        .context("compositor does not offer zwp_input_method_manager_v2")?;
    let vk_manager: ZwpVirtualKeyboardManagerV1 = globals
        .bind(&qh, 1..=1, ())
        .context("compositor does not offer zwp_virtual_keyboard_manager_v1")?;

    let mut state = State::new(
        &qh,
        &seat,
        &compositor,
        &shm,
        fractional_scale.as_ref(),
        viewporter.as_ref(),
        &im_manager,
        &vk_manager,
        engine,
        cfg,
    );
    log::info!("cosmic-ext-ime started ({})", state.engine_name());

    let mut event_loop: EventLoop<'static, State> =
        EventLoop::try_new().context("creating event loop")?;
    let handle = event_loop.handle();
    state.set_loop_handle(handle.clone());
    WaylandSource::new(conn, queue)
        .insert(handle.clone())
        .map_err(|e| anyhow::anyhow!("registering Wayland source: {e}"))?;
    handle
        .insert_source(
            ConfigWatchSource::new(&handler).context("watching config")?,
            |(handler, keys), _, state| {
                log::debug!("config keys changed: {keys:?}");
                state.apply_config(PopeinputConfig::load(&handler));
            },
        )
        .map_err(|e| anyhow::anyhow!("registering config watcher: {e}"))?;

    for (id, version) in [
        (
            cosmic_theme::THEME_MODE_ID,
            cosmic_theme::ThemeMode::VERSION,
        ),
        (cosmic_theme::DARK_THEME_ID, cosmic_theme::Theme::VERSION),
        (cosmic_theme::LIGHT_THEME_ID, cosmic_theme::Theme::VERSION),
    ] {
        let Ok(theme_cfg) = cosmic_config::Config::new(id, version) else {
            continue;
        };
        let Ok(source) = ConfigWatchSource::new(&theme_cfg) else {
            continue;
        };
        handle
            .insert_source(source, |_, _, state| state.reload_theme())
            .map_err(|e| anyhow::anyhow!("registering theme watcher: {e}"))?;
    }

    let signal = event_loop.get_signal();
    event_loop
        .run(None, &mut state, |state| {
            if state.should_exit() {
                signal.stop();
            }
        })
        .context("event loop")?;
    Ok(())
}

/// Toolkits talk to the compositor's text-input protocol only when no IM
/// module is forced; leftover IBus/Fcitx settings silently break that.
fn warn_about_im_modules() {
    for var in ["GTK_IM_MODULE", "QT_IM_MODULE", "XMODIFIERS"] {
        if let Ok(value) = std::env::var(var) {
            if !value.is_empty() && value != "wayland" && value != "none" {
                log::warn!(
                    "{var}={value:?} is set: GTK/Qt apps will use that IM module instead of \
                     Wayland text-input. Remove it from ~/.profile (or run `im-config -n none`) \
                     and log in again."
                );
            }
        }
    }
}
