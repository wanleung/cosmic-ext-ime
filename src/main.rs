mod im;
mod keys;

use anyhow::{Context, Result};
use popeinput_cangjie::{CangjieEngine, Config, Mode};
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::Connection;
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_manager_v2::ZwpInputMethodManagerV2;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;

use crate::im::State;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mode = match std::env::args().nth(1).as_deref() {
        Some("quick") | Some("速成") => Mode::Quick,
        _ => Mode::Cangjie,
    };
    let engine = CangjieEngine::new(Config { mode, ..Config::default() })
        .context("failed to open libcangjie2 database")?;

    let conn = Connection::connect_to_env().context("connecting to Wayland display")?;
    let (globals, mut queue) = registry_queue_init::<State>(&conn)?;
    let qh = queue.handle();

    let seat: WlSeat = globals.bind(&qh, 1..=7, ()).context("wl_seat")?;
    let im_manager: ZwpInputMethodManagerV2 = globals
        .bind(&qh, 1..=1, ())
        .context("compositor does not offer zwp_input_method_manager_v2")?;
    let vk_manager: ZwpVirtualKeyboardManagerV1 = globals
        .bind(&qh, 1..=1, ())
        .context("compositor does not offer zwp_virtual_keyboard_manager_v1")?;

    let mut state = State::new(&qh, &seat, &im_manager, &vk_manager, Box::new(engine));
    log::info!("popeinput started ({})", state.engine_name());

    loop {
        queue.blocking_dispatch(&mut state)?;
        if state.should_exit() {
            break;
        }
    }
    Ok(())
}
