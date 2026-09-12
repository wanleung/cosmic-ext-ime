mod config;
mod im;
mod keys;
mod popup;

use anyhow::{Context, Result};
use popeinput_cangjie::{CangjieEngine, Config as EngineConfig, Mode};
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::{wl_compositor::WlCompositor, wl_seat::WlSeat, wl_shm::WlShm};
use wayland_client::Connection;
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_manager_v2::ZwpInputMethodManagerV2;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;

use crate::config::Config;
use crate::im::State;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cfg = Config::load()?;
    let mode = match std::env::args().nth(1).as_deref() {
        Some("quick") | Some("速成") => Mode::Quick,
        Some("cangjie") | Some("倉頡") => Mode::Cangjie,
        Some(other) => anyhow::bail!("unknown argument {other:?}; expected \"cangjie\" or \"quick\""),
        None => cfg.mode()?,
    };
    let engine = CangjieEngine::new(EngineConfig {
        mode,
        version: cfg.version()?,
        filter: cfg.filter()?,
        page_size: cfg.page_size.clamp(1, 9),
        fullwidth_chars: cfg.fullwidth_chars,
    })
    .context("failed to open libcangjie2 database")?;

    let conn = Connection::connect_to_env().context("connecting to Wayland display")?;
    let (globals, mut queue) = registry_queue_init::<State>(&conn)?;
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

    let mut state = State::new(
        &qh,
        &seat,
        &compositor,
        &shm,
        &im_manager,
        &vk_manager,
        Box::new(engine),
        cfg,
    );
    log::info!("popeinput started ({})", state.engine_name());

    loop {
        queue.blocking_dispatch(&mut state)?;
        if state.should_exit() {
            break;
        }
    }
    Ok(())
}
