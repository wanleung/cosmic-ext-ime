//! Tap keys through a zwp_virtual_keyboard_v1 that shares the seat keymap
//! (no_keymap).  The client is NOT the input-method grab client, so the
//! compositor runs the keys through the full pipeline (seat xkb state,
//! shortcuts, the IM grab).
//!
//! Usage: vktap <evdev-keycode | chord>...  Each key is pressed and released
//! with a pause; "42+38" is a chord (press 42, press 38, release 38, release
//! 42).  Environment:
//! * `VKTAP_DELAY_MS` (default 60): pause after every press and release;
//! * `VKTAP_GAP_MS` (default 150): extra pause after every chord;
//! * `VKTAP_FOCUS_FILE`: before every chord, abort with exit status 2
//!   unless this file reads `1` (improbe's `PROBE_FOCUS_FILE`).
use std::error::Error;
use std::os::fd::AsFd;
use std::time::Duration;

use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_keyboard::KeymapFormat, wl_registry, wl_seat};
use wayland_client::{delegate_noop, Connection, Dispatch, QueueHandle};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

struct App;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for App {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
delegate_noop!(App: ignore wl_seat::WlSeat);
delegate_noop!(App: ignore ZwpVirtualKeyboardManagerV1);
delegate_noop!(App: ignore ZwpVirtualKeyboardV1);

/// Milliseconds from the environment variable `name`, or `default`.
fn env_ms(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn parse_chord(arg: &str) -> Result<Vec<u32>, String> {
    arg.split('+')
        .map(|k| {
            k.parse()
                .map_err(|e| format!("bad evdev keycode {k:?} in {arg:?}: {e}"))
        })
        .collect()
}

fn focused(path: &str) -> bool {
    std::fs::read_to_string(path).is_ok_and(|c| c.trim() == "1")
}

fn main() -> Result<(), Box<dyn Error>> {
    let chords = std::env::args()
        .skip(1)
        .map(|a| parse_chord(&a))
        .collect::<Result<Vec<_>, _>>()?;
    let delay = env_ms("VKTAP_DELAY_MS", 60);
    let gap = env_ms("VKTAP_GAP_MS", 150);
    let focus_file = std::env::var("VKTAP_FOCUS_FILE").ok();

    let conn = Connection::connect_to_env().map_err(|e| format!("wayland: {e}"))?;
    let (globals, mut queue) = registry_queue_init::<App>(&conn)?;
    let qh = queue.handle();
    let seat: wl_seat::WlSeat = globals
        .bind(&qh, 1..=7, ())
        .map_err(|e| format!("wl_seat: {e}"))?;
    let mgr: ZwpVirtualKeyboardManagerV1 = globals
        .bind(&qh, 1..=1, ())
        .map_err(|e| format!("zwp_virtual_keyboard_manager_v1: {e}"))?;
    let vk = mgr.create_virtual_keyboard(&seat, &qh, ());
    // no_keymap shares the seat keymap; the request still needs an fd.
    let dummy = std::fs::File::open("/dev/null")?;
    vk.keymap(KeymapFormat::NoKeymap as u32, dummy.as_fd(), 0);
    queue.roundtrip(&mut App)?;

    let mut time: u32 = 1;
    for (i, chord) in chords.iter().enumerate() {
        if let Some(path) = &focus_file {
            if !focused(path) {
                let unsent: usize = chords[i..].iter().map(Vec::len).sum();
                eprintln!("ABORT: probe window not focused; {unsent} key(s) not sent");
                vk.destroy();
                queue.roundtrip(&mut App).ok();
                std::process::exit(2);
            }
        }
        let presses = chord.iter().map(|&key| (key, 1));
        let releases = chord.iter().rev().map(|&key| (key, 0));
        for (key, state) in presses.chain(releases) {
            vk.key(time, key, state);
            queue.roundtrip(&mut App)?;
            std::thread::sleep(Duration::from_millis(delay));
            time += delay as u32;
        }
        std::thread::sleep(Duration::from_millis(gap));
        time += gap as u32;
    }
    vk.destroy();
    queue.roundtrip(&mut App)?;
    println!("ok");
    Ok(())
}
