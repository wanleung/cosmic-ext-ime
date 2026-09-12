//! Candidate window drawn into a wl_shm buffer and shown through
//! zwp_input_popup_surface_v2, which the compositor positions at the caret.

use std::fs::File;
use std::os::fd::AsFd;

use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Weight};
use memmap2::MmapMut;
use popeinput_engine::{Candidate, PageInfo};
use wayland_client::protocol::{wl_buffer, wl_compositor, wl_shm, wl_shm_pool, wl_surface};
use wayland_client::QueueHandle;
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_v2::ZwpInputMethodV2, zwp_input_popup_surface_v2::ZwpInputPopupSurfaceV2,
};

const PADDING: i32 = 8;
const BORDER: i32 = 1;

pub struct Style {
    pub font_size: f32,
    pub background: [u8; 3],
    pub foreground: [u8; 3],
    pub hint: [u8; 3],
    pub border: [u8; 3],
}

impl Style {
    fn new(font_size: f32) -> Self {
        Style {
            font_size: font_size.clamp(6.0, 72.0),
            background: [0x1e, 0x1e, 0x1e],
            foreground: [0xf0, 0xf0, 0xf0],
            hint: [0x8a, 0x8a, 0x8a],
            border: [0x5a, 0x5a, 0x5a],
        }
    }
}

pub struct Popup<S: 'static> {
    shm: wl_shm::WlShm,
    surface: wl_surface::WlSurface,
    _popup: ZwpInputPopupSurfaceV2,
    qh: QueueHandle<S>,
    font_system: FontSystem,
    swash_cache: SwashCache,
    style: Style,
    visible: bool,
}

impl<S> Popup<S>
where
    S: wayland_client::Dispatch<wl_surface::WlSurface, ()>
        + wayland_client::Dispatch<wl_shm_pool::WlShmPool, ()>
        + wayland_client::Dispatch<wl_buffer::WlBuffer, ()>
        + wayland_client::Dispatch<ZwpInputPopupSurfaceV2, ()>
        + 'static,
{
    pub fn new(
        qh: &QueueHandle<S>,
        compositor: &wl_compositor::WlCompositor,
        shm: &wl_shm::WlShm,
        input_method: &ZwpInputMethodV2,
        font_size: f32,
    ) -> Self {
        let surface = compositor.create_surface(qh, ());
        let popup = input_method.get_input_popup_surface(&surface, qh, ());
        let mut font_system = FontSystem::new();
        font_system.db_mut().set_sans_serif_family("Noto Sans CJK HK");
        Popup {
            shm: shm.clone(),
            surface,
            _popup: popup,
            qh: qh.clone(),
            font_system,
            swash_cache: SwashCache::new(),
            style: Style::new(font_size),
            visible: false,
        }
    }

    pub fn set_font_size(&mut self, font_size: f32) {
        self.style = Style::new(font_size);
    }

    pub fn hide(&mut self) {
        if self.visible {
            self.surface.attach(None, 0, 0);
            self.surface.commit();
            self.visible = false;
        }
    }

    /// Show `header` (the typed radicals) above the numbered candidates.
    pub fn show(&mut self, header: &str, candidates: &[Candidate], page: PageInfo) {
        if header.is_empty() && candidates.is_empty() {
            self.hide();
            return;
        }

        let fg = Color::rgb(self.style.foreground[0], self.style.foreground[1], self.style.foreground[2]);
        let hint = Color::rgb(self.style.hint[0], self.style.hint[1], self.style.hint[2]);
        let base = Attrs::new().family(Family::SansSerif);

        let mut spans: Vec<(String, Attrs)> = Vec::new();
        spans.push((header.to_string(), base.clone().color(fg).weight(Weight::BOLD)));
        for (i, c) in candidates.iter().enumerate() {
            spans.push((format!("\n{}. {}", i + 1, c.text), base.clone().color(fg)));
            if let Some(h) = &c.hint {
                spans.push((format!("  {h}"), base.clone().color(hint)));
            }
        }
        let paging = match page.total_pages {
            Some(total) if total > 1 => Some(format!("\n{}/{}", page.page + 1, total)),
            None if page.page > 0 || page.has_next => Some(format!(
                "\n{}{}{}",
                if page.page > 0 { "◂ " } else { "" },
                page.page + 1,
                if page.has_next { " ▸" } else { "" }
            )),
            _ => None,
        };
        if let Some(p) = paging {
            spans.push((p, base.clone().color(hint)));
        }

        let metrics = Metrics::new(self.style.font_size, self.style.font_size * 1.5);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(None, None);
        buffer.set_rich_text(
            spans.iter().map(|(s, a)| (s.as_str(), a.clone())),
            &base,
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut text_w: f32 = 0.0;
        let mut lines = 0;
        for run in buffer.layout_runs() {
            text_w = text_w.max(run.line_w);
            lines += 1;
        }
        let width = text_w.ceil() as i32 + 2 * (PADDING + BORDER);
        let height = (lines as f32 * metrics.line_height).ceil() as i32 + 2 * (PADDING + BORDER);

        let mut canvas = match Canvas::new(&self.shm, &self.qh, width, height) {
            Ok(c) => c,
            Err(e) => {
                log::error!("popup buffer: {e}");
                return;
            }
        };
        canvas.fill(self.style.background);
        canvas.border(self.style.border);
        let origin = PADDING + BORDER;
        buffer.draw(&mut self.font_system, &mut self.swash_cache, fg, |x, y, w, h, color| {
            canvas.blend_rect(x + origin, y + origin, w as i32, h as i32, color);
        });

        self.surface.attach(Some(&canvas.buffer), 0, 0);
        self.surface.damage_buffer(0, 0, width, height);
        self.surface.commit();
        self.visible = true;
        // The compositor releases the buffer when done; wl_buffer::Event::Release
        // destroys it (see the Dispatch impl in im.rs), and the mmap is dropped here.
    }
}

struct Canvas {
    buffer: wl_buffer::WlBuffer,
    map: MmapMut,
    width: i32,
    height: i32,
}

impl Canvas {
    fn new<S>(shm: &wl_shm::WlShm, qh: &QueueHandle<S>, width: i32, height: i32) -> anyhow::Result<Self>
    where
        S: wayland_client::Dispatch<wl_shm_pool::WlShmPool, ()>
            + wayland_client::Dispatch<wl_buffer::WlBuffer, ()>
            + 'static,
    {
        let stride = width * 4;
        let size = (stride * height) as usize;
        let fd = rustix::fs::memfd_create("popeinput-popup", rustix::fs::MemfdFlags::CLOEXEC)?;
        let file = File::from(fd);
        file.set_len(size as u64)?;
        // SAFETY: the file is private to us and sized above; the compositor only reads it.
        let map = unsafe { MmapMut::map_mut(&file)? };
        let pool = shm.create_pool(file.as_fd(), size as i32, qh, ());
        let buffer = pool.create_buffer(0, width, height, stride, wl_shm::Format::Argb8888, qh, ());
        pool.destroy();
        Ok(Canvas { buffer, map, width, height })
    }

    fn fill(&mut self, rgb: [u8; 3]) {
        for px in self.map.as_chunks_mut::<4>().0 {
            *px = [rgb[2], rgb[1], rgb[0], 0xff];
        }
    }

    fn border(&mut self, rgb: [u8; 3]) {
        let (w, h) = (self.width, self.height);
        for x in 0..w {
            for y in [0, h - 1] {
                self.put(x, y, rgb);
            }
        }
        for y in 0..h {
            for x in [0, w - 1] {
                self.put(x, y, rgb);
            }
        }
    }

    fn put(&mut self, x: i32, y: i32, rgb: [u8; 3]) {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return;
        }
        let i = ((y * self.width + x) * 4) as usize;
        self.map[i..i + 4].copy_from_slice(&[rgb[2], rgb[1], rgb[0], 0xff]);
    }

    fn blend_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        let a = color.a() as u32;
        if a == 0 {
            return;
        }
        let (r, g, b) = (color.r() as u32, color.g() as u32, color.b() as u32);
        for yy in y.max(0)..(y + h).min(self.height) {
            for xx in x.max(0)..(x + w).min(self.width) {
                let i = ((yy * self.width + xx) * 4) as usize;
                let px = &mut self.map[i..i + 4];
                px[0] = ((b * a + px[0] as u32 * (255 - a)) / 255) as u8;
                px[1] = ((g * a + px[1] as u32 * (255 - a)) / 255) as u8;
                px[2] = ((r * a + px[2] as u32 * (255 - a)) / 255) as u8;
                px[3] = 0xff;
            }
        }
    }
}
