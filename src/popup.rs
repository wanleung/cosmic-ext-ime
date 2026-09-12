//! Candidate window drawn into a wl_shm buffer and shown through
//! zwp_input_popup_surface_v2, which the compositor positions at the caret.
//! Styled like a COSMIC dropdown popover and rendered at the output's
//! (fractional) scale.

use std::fs::File;
use std::os::fd::AsFd;

use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Weight};
use memmap2::MmapMut;
use popeinput_engine::{Candidate, PageInfo};
use wayland_client::protocol::{wl_buffer, wl_compositor, wl_shm, wl_shm_pool, wl_surface};
use wayland_client::QueueHandle;
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
    wp_fractional_scale_v1::WpFractionalScaleV1,
};
use wayland_protocols::wp::viewporter::client::{
    wp_viewport::WpViewport, wp_viewporter::WpViewporter,
};
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_v2::ZwpInputMethodV2, zwp_input_popup_surface_v2::ZwpInputPopupSurfaceV2,
};

/// Logical-pixel metrics, multiplied by the scale when drawing.
const PADDING: f32 = 8.0;
const BORDER: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba(pub [f32; 4]);

impl Rgba {
    fn premultiplied_bytes(self) -> [u8; 4] {
        let [r, g, b, a] = self.0;
        [
            (b * a * 255.0) as u8,
            (g * a * 255.0) as u8,
            (r * a * 255.0) as u8,
            (a * 255.0) as u8,
        ]
    }

    fn text_color(self) -> Color {
        let [r, g, b, a] = self.0;
        Color::rgba(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
            (a * 255.0) as u8,
        )
    }

    fn with_alpha(self, a: f32) -> Rgba {
        Rgba([self.0[0], self.0[1], self.0[2], a])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub font_size: f32,
    pub background: Rgba,
    pub foreground: Rgba,
    pub hint: Rgba,
    pub border: Rgba,
    pub highlight: Rgba,
    pub radius: f32,
}

impl Style {
    /// Mirror libcosmic's `Container::Dropdown` look for the active theme.
    pub fn from_cosmic(font_size: f32) -> Self {
        let theme = cosmic_theme::Theme::get_active().unwrap_or_else(|(errors, t)| {
            log::debug!("using default COSMIC theme: {errors:?}");
            t
        });
        let c = |s: cosmic_theme::palette::Srgba| Rgba([s.red, s.green, s.blue, s.alpha]);
        let foreground = c(theme.on_bg_component_color());
        Style {
            font_size: font_size.clamp(6.0, 72.0),
            background: c(theme.bg_component_color()).with_alpha(1.0),
            foreground,
            hint: foreground.with_alpha(0.6),
            border: c(theme.bg_component_divider()),
            highlight: c(theme.accent_color()),
            radius: theme.corner_radii.radius_s[0],
        }
    }
}

pub struct Popup<S: 'static> {
    shm: wl_shm::WlShm,
    surface: wl_surface::WlSurface,
    _popup: ZwpInputPopupSurfaceV2,
    _fractional: Option<WpFractionalScaleV1>,
    viewport: Option<WpViewport>,
    qh: QueueHandle<S>,
    font_system: FontSystem,
    swash_cache: SwashCache,
    style: Style,
    scale: f32,
    visible: bool,
    /// Last shown content, redrawn when the scale or style changes.
    last: Option<(String, Vec<Candidate>, PageInfo, usize)>,
}

impl<S> Popup<S>
where
    S: wayland_client::Dispatch<wl_surface::WlSurface, ()>
        + wayland_client::Dispatch<wl_shm_pool::WlShmPool, ()>
        + wayland_client::Dispatch<wl_buffer::WlBuffer, ()>
        + wayland_client::Dispatch<ZwpInputPopupSurfaceV2, ()>
        + wayland_client::Dispatch<WpFractionalScaleV1, ()>
        + wayland_client::Dispatch<WpViewport, ()>
        + 'static,
{
    pub fn new(
        qh: &QueueHandle<S>,
        compositor: &wl_compositor::WlCompositor,
        shm: &wl_shm::WlShm,
        fractional_manager: Option<&WpFractionalScaleManagerV1>,
        viewporter: Option<&WpViewporter>,
        input_method: &ZwpInputMethodV2,
        style: Style,
    ) -> Self {
        let surface = compositor.create_surface(qh, ());
        let popup = input_method.get_input_popup_surface(&surface, qh, ());
        let fractional = fractional_manager.map(|m| m.get_fractional_scale(&surface, qh, ()));
        let viewport = viewporter.map(|v| v.get_viewport(&surface, qh, ()));
        if fractional.is_none() || viewport.is_none() {
            log::info!("compositor lacks fractional scaling; popup falls back to integer scale");
        }
        let mut font_system = FontSystem::new();
        font_system
            .db_mut()
            .set_sans_serif_family("Noto Sans CJK HK");
        Popup {
            shm: shm.clone(),
            surface,
            _popup: popup,
            _fractional: fractional,
            viewport,
            qh: qh.clone(),
            font_system,
            swash_cache: SwashCache::new(),
            style,
            scale: 1.0,
            visible: false,
            last: None,
        }
    }

    pub fn set_style(&mut self, style: Style) {
        if self.style != style {
            self.style = style;
            self.redraw();
        }
    }

    /// From wp_fractional_scale_v1 (scale × 120) or wl_surface's integer hint.
    pub fn set_scale(&mut self, scale: f32) {
        if (self.scale - scale).abs() > f32::EPSILON {
            log::debug!("popup scale {scale}");
            self.scale = scale;
            self.redraw();
        }
    }

    fn redraw(&mut self) {
        if let Some((header, candidates, page, selected)) = self.last.take() {
            if self.visible {
                self.show(&header, &candidates, page, selected);
            }
        }
    }

    pub fn hide(&mut self) {
        self.last = None;
        if self.visible {
            self.surface.attach(None, 0, 0);
            self.surface.commit();
            self.visible = false;
        }
    }

    /// Show `header` (the typed radicals) above the numbered candidates.
    pub fn show(
        &mut self,
        header: &str,
        candidates: &[Candidate],
        page: PageInfo,
        selected: usize,
    ) {
        if header.is_empty() && candidates.is_empty() {
            self.hide();
            return;
        }
        self.last = Some((header.to_string(), candidates.to_vec(), page, selected));

        let scale = self.scale;
        let fg = self.style.foreground.text_color();
        let hint = self.style.hint.text_color();
        let highlight = self.style.highlight.text_color();
        let base = Attrs::new().family(Family::SansSerif);

        let mut spans: Vec<(String, Attrs)> = Vec::new();
        spans.push((
            header.to_string(),
            base.clone().color(fg).weight(Weight::BOLD),
        ));
        for (i, c) in candidates.iter().enumerate() {
            let color = if i == selected && candidates.len() > 1 {
                highlight
            } else {
                fg
            };
            spans.push((
                format!("\n{}. {}", i + 1, c.text),
                base.clone().color(color),
            ));
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

        let font_px = self.style.font_size * scale;
        let metrics = Metrics::new(font_px, font_px * 1.5);
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
        let inset = (PADDING + BORDER) * scale;
        let logical_w = ((text_w + 2.0 * inset) / scale).ceil() as i32;
        let logical_h = ((lines as f32 * metrics.line_height + 2.0 * inset) / scale).ceil() as i32;
        let width = (logical_w as f32 * scale).round() as i32;
        let height = (logical_h as f32 * scale).round() as i32;

        let mut canvas = match Canvas::new(&self.shm, &self.qh, width, height) {
            Ok(c) => c,
            Err(e) => {
                log::error!("popup buffer: {e}");
                return;
            }
        };
        canvas.fill_rounded(
            self.style.background,
            self.style.border,
            BORDER * scale,
            self.style.radius * scale,
        );
        let origin = inset.round() as i32;
        buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            fg,
            |x, y, w, h, color| {
                canvas.blend_rect(x + origin, y + origin, w as i32, h as i32, color);
            },
        );

        match &self.viewport {
            Some(viewport) => viewport.set_destination(logical_w, logical_h),
            None => self.surface.set_buffer_scale(scale.round().max(1.0) as i32),
        }
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
    fn new<S>(
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<S>,
        width: i32,
        height: i32,
    ) -> anyhow::Result<Self>
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
        Ok(Canvas {
            buffer,
            map,
            width,
            height,
        })
    }

    /// Rounded rectangle with a border; outside the shape stays transparent.
    fn fill_rounded(&mut self, fill: Rgba, border: Rgba, border_w: f32, radius: f32) {
        let (w, h) = (self.width as f32, self.height as f32);
        let r = radius.min(w / 2.0).min(h / 2.0);
        let fill_px = fill.premultiplied_bytes();
        let border_px = border.premultiplied_bytes();
        for y in 0..self.height {
            for x in 0..self.width {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                // Signed distance from the rounded-rect edge (negative inside).
                let dx = (px - w / 2.0).abs() - (w / 2.0 - r);
                let dy = (py - h / 2.0).abs() - (h / 2.0 - r);
                let d =
                    (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt() + dx.max(dy).min(0.0) - r;
                if d > 0.0 {
                    continue;
                }
                let color = if d > -border_w { border_px } else { fill_px };
                let i = ((y * self.width + x) * 4) as usize;
                self.map[i..i + 4].copy_from_slice(&color);
            }
        }
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
                // Premultiplied "over" onto the existing pixel.
                px[0] = ((b * a + px[0] as u32 * (255 - a)) / 255) as u8;
                px[1] = ((g * a + px[1] as u32 * (255 - a)) / 255) as u8;
                px[2] = ((r * a + px[2] as u32 * (255 - a)) / 255) as u8;
                px[3] = (a + px[3] as u32 * (255 - a) / 255).min(255) as u8;
            }
        }
    }
}
