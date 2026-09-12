mod config;
mod im;
mod keys;
mod popup;

use anyhow::{Context, Result};
use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use cosmic_config::calloop::ConfigWatchSource;
use popeinput_config::PopeinputConfig;
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::{wl_compositor::WlCompositor, wl_seat::WlSeat, wl_shm::WlShm};
use wayland_client::Connection;
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_manager_v2::ZwpInputMethodManagerV2;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;

use crate::im::State;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

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
    let im_manager: ZwpInputMethodManagerV2 = globals
        .bind(&qh, 1..=1, ())
        .context("compositor does not offer zwp_input_method_manager_v2")?;
    let vk_manager: ZwpVirtualKeyboardManagerV1 = globals
        .bind(&qh, 1..=1, ())
        .context("compositor does not offer zwp_virtual_keyboard_manager_v1")?;

    let mut state = State::new(&qh, &seat, &compositor, &shm, &im_manager, &vk_manager, engine, cfg);
    log::info!("popeinput started ({})", state.engine_name());

    let mut event_loop: EventLoop<State> = EventLoop::try_new().context("creating event loop")?;
    let handle = event_loop.handle();
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
