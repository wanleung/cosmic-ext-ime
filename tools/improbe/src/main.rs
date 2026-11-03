//! A visible text-field window: maps a toplevel, enables zwp_text_input_v3 on
//! keyboard focus, and DRAWS what an application receives — forwarded keys
//! resolved through its own xkb state (like a real toolkit) and text-input
//! commits, with the live preedit in blue.  Every event is also logged to
//! stdout; `tools/run_line.sh` parses that log.
//!
//! Usage: improbe [seconds]   (default 20; the window then closes itself).
//! `PROBE_FOCUS_FILE`, when set, is kept at "1" while the window has keyboard
//! focus and "0" otherwise.
use std::error::Error;
use std::os::fd::AsFd;
use std::time::{Duration, Instant};

use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache};
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_keyboard, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
};
use wayland_client::{delegate_noop, Connection, Dispatch, QueueHandle, WEnum};
use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3::ZwpTextInputManagerV3,
    zwp_text_input_v3::{self, ZwpTextInputV3},
};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};
use xkbcommon::xkb;

const W: i32 = 1100;
const H: i32 = 380;
/// Text inset from the window edge, in pixels.
const MARGIN: i32 = 20;

struct App {
    shm: wl_shm::WlShm,
    surface: wl_surface::WlSurface,
    configured: bool,
    text_input: Option<ZwpTextInputV3>,
    ctx: xkb::Context,
    state: Option<xkb::State>,
    closed: bool,
    /// Everything received so far: forwarded keys and commits.
    doc: String,
    preedit: String,
    dirty: bool,
    font_system: FontSystem,
    swash: SwashCache,
    t0: Instant,
}

/// Record keyboard focus in `PROBE_FOCUS_FILE`, if set.
fn write_focus_file(focused: bool) {
    if let Ok(path) = std::env::var("PROBE_FOCUS_FILE") {
        if let Err(e) = std::fs::write(&path, if focused { "1" } else { "0" }) {
            eprintln!("writing {path}: {e}");
        }
    }
}

impl App {
    /// Elapsed time, as the prefix of every log line.
    fn ts(&self) -> String {
        format!("{:6.2}s", self.t0.elapsed().as_secs_f32())
    }

    fn redraw(&mut self, qh: &QueueHandle<Self>) {
        self.dirty = false;
        if let Err(e) = self.draw_frame(qh) {
            eprintln!("{} redraw failed: {e}", self.ts());
        }
    }

    /// Render the document into a fresh shm buffer and commit it.
    fn draw_frame(&mut self, qh: &QueueHandle<Self>) -> std::io::Result<()> {
        let size = (W * H * 4) as usize;
        let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
        let path = format!(
            "{dir}/improbe-{}-{}.shm",
            std::process::id(),
            self.t0.elapsed().as_micros()
        );
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;
        // Only the fd is needed from here on.
        let _ = std::fs::remove_file(&path);
        file.set_len(size as u64)?;
        // SAFETY: the file was just created and unlinked by this process and
        // is not resized while mapped; the compositor only reads it.
        let mut map = unsafe { memmap2::MmapMut::map_mut(&file) }?;
        for px in map.chunks_exact_mut(4) {
            px.copy_from_slice(&[0xf8, 0xf8, 0xf8, 0xff]);
        }

        let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(26.0, 36.0));
        buffer.set_size(Some((W - 2 * MARGIN) as f32), Some((H - 2 * MARGIN) as f32));
        let header = format!(
            "cosmic-ext-ime probe — {} chars received\n",
            self.doc.chars().count()
        );
        let grey = Attrs::new().color(Color::rgb(0x80, 0x80, 0x80));
        let black = Attrs::new().color(Color::rgb(0x10, 0x10, 0x10));
        let blue = Attrs::new().color(Color::rgb(0x10, 0x40, 0xd0));
        let spans = [
            (header.as_str(), grey),
            (self.doc.as_str(), black.clone()),
            (self.preedit.as_str(), blue),
        ];
        buffer.set_rich_text(
            spans.iter().map(|(s, a)| (*s, a.clone())),
            &black,
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut self.font_system, true);
        buffer.draw(
            &mut self.font_system,
            &mut self.swash,
            Color::rgb(0, 0, 0),
            |x, y, w, h, color| {
                let a = color.a() as u32;
                if a == 0 {
                    return;
                }
                let blend =
                    |dst: u8, src: u8| ((src as u32 * a + dst as u32 * (255 - a)) / 255) as u8;
                for py in (y + MARGIN..y + MARGIN + h as i32).filter(|py| (0..H).contains(py)) {
                    for px in (x + MARGIN..x + MARGIN + w as i32).filter(|px| (0..W).contains(px)) {
                        // Xrgb8888 is little-endian B, G, R, X.
                        let i = ((py * W + px) * 4) as usize;
                        map[i] = blend(map[i], color.b());
                        map[i + 1] = blend(map[i + 1], color.g());
                        map[i + 2] = blend(map[i + 2], color.r());
                    }
                }
            },
        );

        let pool = self.shm.create_pool(file.as_fd(), size as i32, qh, ());
        let wbuf = pool.create_buffer(0, W, H, W * 4, wl_shm::Format::Xrgb8888, qh, ());
        pool.destroy();
        self.surface.attach(Some(&wbuf), 0, 0);
        self.surface.damage_buffer(0, 0, W, H);
        self.surface.commit();
        // `map` and `file` drop here; the compositor keeps its own mapping.
        Ok(())
    }
}

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

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for App {
    fn event(
        _: &mut Self,
        wm: &xdg_wm_base::XdgWmBase,
        e: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = e {
            wm.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for App {
    fn event(
        app: &mut Self,
        xs: &xdg_surface::XdgSurface,
        e: xdg_surface::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = e {
            println!("{} xdg configure", app.ts());
            xs.ack_configure(serial);
            app.configured = true;
            app.redraw(qh);
            println!("{} first frame committed", app.ts());
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for App {
    fn event(
        app: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        e: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_toplevel::Event::Close = e {
            app.closed = true;
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for App {
    fn event(
        _: &mut Self,
        seat: &wl_seat::WlSeat,
        e: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(caps),
        } = e
        {
            if caps.contains(wl_seat::Capability::Keyboard) {
                seat.get_keyboard(qh, ());
            }
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for App {
    fn event(
        app: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        e: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use wl_keyboard::Event;
        match e {
            Event::Keymap { fd, size, .. } => {
                // SAFETY: the compositor sends a readable keymap of `size`
                // bytes.
                let keymap = unsafe {
                    xkb::Keymap::new_from_fd(
                        &app.ctx,
                        fd,
                        size as usize,
                        xkb::KEYMAP_FORMAT_TEXT_V1,
                        xkb::KEYMAP_COMPILE_NO_FLAGS,
                    )
                };
                match keymap {
                    Ok(Some(keymap)) => {
                        println!("{} kbd keymap {size}B", app.ts());
                        app.state = Some(xkb::State::new(&keymap));
                    }
                    Ok(None) => eprintln!("{} kbd keymap failed to compile", app.ts()),
                    Err(e) => eprintln!("{} kbd keymap unreadable: {e}", app.ts()),
                }
            }
            Event::Enter { .. } => {
                println!("{} kbd ENTER (focus)", app.ts());
                write_focus_file(true);
                if let Some(ti) = &app.text_input {
                    ti.enable();
                    ti.set_content_type(
                        zwp_text_input_v3::ContentHint::None,
                        zwp_text_input_v3::ContentPurpose::Normal,
                    );
                    ti.set_cursor_rectangle(20, 60, 2, 30);
                    ti.commit();
                }
            }
            Event::Leave { .. } => {
                println!("{} kbd LEAVE", app.ts());
                write_focus_file(false);
            }
            Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                if let Some(st) = app.state.as_mut() {
                    st.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);
                }
                println!(
                    "{} kbd modifiers depressed={mods_depressed:#x} latched={mods_latched:#x} \
                     locked={mods_locked:#x} group={group}",
                    app.ts()
                );
            }
            Event::Key { key, state, .. } => {
                let (sym, utf8) = match app.state.as_ref() {
                    Some(st) => {
                        let kc = xkb::Keycode::new(key + 8);
                        (
                            xkb::keysym_get_name(st.key_get_one_sym(kc)),
                            st.key_get_utf8(kc),
                        )
                    }
                    None => ("?".into(), String::new()),
                };
                println!(
                    "{} kbd key {key} {state:?} -> sym {sym} utf8 {utf8:?}",
                    app.ts()
                );
                if state == WEnum::Value(wl_keyboard::KeyState::Pressed) {
                    if sym == "BackSpace" {
                        app.doc.pop();
                    } else if sym == "Return" {
                        app.doc.push('\n');
                    } else if !utf8.is_empty() && !utf8.chars().any(char::is_control) {
                        app.doc.push_str(&utf8);
                    } else {
                        return;
                    }
                    app.dirty = true;
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwpTextInputV3, ()> for App {
    fn event(
        app: &mut Self,
        _: &ZwpTextInputV3,
        e: zwp_text_input_v3::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwp_text_input_v3::Event;
        match e {
            Event::Enter { .. } => println!("{} text-input enter", app.ts()),
            Event::Leave { .. } => println!("{} text-input leave", app.ts()),
            Event::PreeditString {
                text,
                cursor_begin,
                cursor_end,
            } => {
                println!(
                    "{} text-input preedit {text:?} [{cursor_begin},{cursor_end}]",
                    app.ts()
                );
                app.preedit = text.unwrap_or_default();
                app.dirty = true;
            }
            Event::CommitString { text } => {
                println!("{} text-input COMMIT {text:?}", app.ts());
                if let Some(t) = text {
                    app.doc.push_str(&t);
                    app.dirty = true;
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for App {
    fn event(
        _: &mut Self,
        b: &wl_buffer::WlBuffer,
        e: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = e {
            b.destroy();
        }
    }
}

delegate_noop!(App: ignore wl_compositor::WlCompositor);
delegate_noop!(App: ignore wl_shm::WlShm);
delegate_noop!(App: ignore wl_shm_pool::WlShmPool);
delegate_noop!(App: ignore wl_surface::WlSurface);
delegate_noop!(App: ignore ZwpTextInputManagerV3);

fn main() -> Result<(), Box<dyn Error>> {
    let secs: u64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(20);
    let conn = Connection::connect_to_env().map_err(|e| format!("wayland: {e}"))?;
    let (globals, mut queue) = registry_queue_init::<App>(&conn)?;
    let qh = queue.handle();
    let compositor: wl_compositor::WlCompositor = globals
        .bind(&qh, 4..=6, ())
        .map_err(|e| format!("wl_compositor: {e}"))?;
    let shm: wl_shm::WlShm = globals
        .bind(&qh, 1..=1, ())
        .map_err(|e| format!("wl_shm: {e}"))?;
    let wm: xdg_wm_base::XdgWmBase = globals
        .bind(&qh, 1..=6, ())
        .map_err(|e| format!("xdg_wm_base: {e}"))?;
    let seat: wl_seat::WlSeat = globals
        .bind(&qh, 1..=7, ())
        .map_err(|e| format!("wl_seat: {e}"))?;
    // Without text-input the probe still logs forwarded keys.
    let ti_mgr: Option<ZwpTextInputManagerV3> = globals.bind(&qh, 1..=1, ()).ok();

    let surface = compositor.create_surface(&qh, ());
    let xdg_surface = wm.get_xdg_surface(&surface, &qh, ());
    let toplevel = xdg_surface.get_toplevel(&qh, ());
    toplevel.set_title("cosmic-ext-ime probe (auto-closes)".into());
    toplevel.set_app_id("io.github.wanleung.CosmicExtIme.probe".into());
    surface.commit();
    let text_input = ti_mgr.map(|m| m.get_text_input(&seat, &qh, ()));
    let mut app = App {
        shm,
        surface,
        configured: false,
        text_input,
        ctx: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
        state: None,
        closed: false,
        doc: String::new(),
        preedit: String::new(),
        dirty: false,
        font_system: FontSystem::new(),
        swash: SwashCache::new(),
        t0: Instant::now(),
    };

    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline && !app.closed {
        queue.flush().ok();
        if let Some(guard) = queue.prepare_read() {
            // Nothing to read yet is not an error; the loop polls.
            let _ = guard.read();
        }
        queue.dispatch_pending(&mut app)?;
        if app.dirty && app.configured {
            app.redraw(&qh);
            queue.flush().ok();
        }
        std::thread::sleep(Duration::from_millis(8));
    }
    write_focus_file(false);
    println!("DOC: {}", app.doc.replace('\n', "⏎"));
    Ok(())
}
