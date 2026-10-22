use std::ffi::{c_int, c_void};
use std::ptr::NonNull;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use font_kit::font::Font;
use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;
use wgpu::rwh::{
    DisplayHandle, HandleError, HasDisplayHandle, RawDisplayHandle, RawWindowHandle,
    UiKitDisplayHandle, UiKitWindowHandle,
};

use self::cursor_motion::CursorMotion;
use super::terminal_effects::TerminalEffects;
use super::{
    TerminalCursorMotion, TerminalFontFamily, TerminalFrame, TerminalRenderSettings,
    TerminalVisualEffect,
};
use crate::Scene;
use crate::fonts::FontId;
use crate::fonts::font_kit::Rasterizer;
use crate::rendering::wgpu::{Renderer as WgpuRenderer, Resources, init_wgpu_instance};
use crate::rendering::{Config, GPUPowerPreference, GlyphConfig};

const HORIZONTAL_PADDING: f32 = 4.0;
const VERTICAL_PADDING: f32 = 6.0;
const CURSOR_IDLE_DELAY: Duration = Duration::from_millis(500);
const CURSOR_BLINK_HALF_PERIOD: Duration = Duration::from_millis(500);

mod cursor_motion;
mod draw;

unsafe extern "C" {
    fn pthread_main_np() -> c_int;
}

#[derive(Debug)]
struct UiKitDisplay;

// UIKit's raw display handle contains no borrowed data. This provider is deliberately separate
// from the main-thread-only UIView window handle.
unsafe impl Send for UiKitDisplay {}
unsafe impl Sync for UiKitDisplay {}

impl HasDisplayHandle for UiKitDisplay {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::uikit())
    }
}

pub struct Renderer {
    resources: Resources,
    renderer: WgpuRenderer,
    size_points: Vector2F,
    display_scale: f32,
    fonts: Vec<TerminalFont>,
    metrics: TerminalMetrics,
    rasterizer: Rasterizer,
    terminal_frame: TerminalFrame,
    settings: TerminalRenderSettings,
    effects: TerminalEffects,
    cursor_motion: Option<CursorMotion>,
    pending_cursor_duration_scale: Option<f32>,
    cursor_visible: bool,
    last_activity: Instant,
}

struct TerminalFont {
    id: FontId,
    font: Arc<Font>,
}

#[derive(Clone, Copy)]
struct TerminalMetrics {
    cell_width: f32,
    line_height: f32,
    baseline: f32,
}

impl TerminalMetrics {
    fn new(
        font: &Font,
        display_scale: f32,
        font_size: f32,
        line_height_ratio: f32,
    ) -> Result<Self> {
        let font_metrics = font.metrics();
        let units_per_em = font_metrics.units_per_em as f32;
        if units_per_em <= 0.0 {
            bail!("terminal font has invalid units per em");
        }
        let glyph = font
            .glyph_for_char('M')
            .ok_or_else(|| anyhow!("terminal font is missing the measurement glyph"))?;
        let font_scale = font_size / units_per_em;
        let cell_width = pixel_ceil(font.advance(glyph)?.x() * font_scale, display_scale);
        let ascent = font_metrics.ascent * font_scale;
        let descent = (-font_metrics.descent * font_scale).max(0.0);
        let line_gap = (font_metrics.line_gap * font_scale).max(0.0);
        let line_height = pixel_ceil(
            (ascent + descent + line_gap).max(font_size * line_height_ratio),
            display_scale,
        );
        let baseline = pixel_round(
            (line_height - ascent - descent) * 0.5 + ascent,
            display_scale,
        );
        Ok(Self {
            cell_width,
            line_height,
            baseline,
        })
    }
}

impl Renderer {
    /// Creates a WarpUI renderer for a live UIKit view.
    ///
    /// # Safety
    ///
    /// `ui_view` must point to a live `UIView` backed by a `CAMetalLayer`. The view must outlive
    /// this renderer, and every method including destruction must run on the main thread.
    pub unsafe fn new(
        ui_view: NonNull<c_void>,
        width_points: f32,
        height_points: f32,
        display_scale: f32,
        font_bytes: &[&[u8]],
        settings: TerminalRenderSettings,
    ) -> Result<Self> {
        ensure_main_thread()?;
        let size_points = validated_size(width_points, height_points, display_scale)?;
        let pixel_size = size_points * display_scale;

        init_wgpu_instance(Box::new(UiKitDisplay));
        let raw_display_handle: RawDisplayHandle = UiKitDisplayHandle::new().into();
        let raw_window_handle: RawWindowHandle = UiKitWindowHandle::new(ui_view).into();
        // SAFETY: The Swift host owns the UIView until after the runtime handle is destroyed, and
        // this adapter rejects construction and use away from the UIKit main thread.
        let resources = unsafe {
            Resources::new_from_raw_handles(
                Some(raw_display_handle),
                raw_window_handle,
                GPUPowerPreference::LowPower,
                Some(wgpu::Backend::Metal),
                &|_| {},
                pixel_size,
                false,
            )?
        };
        let renderer = WgpuRenderer::new(&resources, GlyphConfig::default());
        let mut fonts = font_bytes
            .iter()
            .enumerate()
            .map(|(index, bytes)| {
                Font::from_bytes(Arc::new(bytes.to_vec()), 0).map(|font| TerminalFont {
                    id: FontId(index),
                    font: Arc::new(font),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if fonts.is_empty() {
            bail!("terminal renderer requires at least one font");
        }
        for font in load_system_fallback_fonts(settings.font_size) {
            fonts.push(TerminalFont {
                id: FontId(fonts.len()),
                font,
            });
        }
        let primary_font_index = primary_font_index(settings.font_family, fonts.len());
        let metrics = TerminalMetrics::new(
            &fonts[primary_font_index].font,
            display_scale,
            settings.font_size,
            settings.line_height_ratio,
        )?;
        let rasterizer = Rasterizer::new();
        for font in &fonts {
            rasterizer.insert(font.id, font.font.clone());
        }
        let mut this = Self {
            resources,
            renderer,
            size_points,
            display_scale,
            fonts,
            metrics,
            rasterizer,
            terminal_frame: TerminalFrame::default(),
            settings,
            effects: TerminalEffects::new(),
            cursor_motion: None,
            pending_cursor_duration_scale: None,
            cursor_visible: true,
            last_activity: Instant::now(),
        };
        this.render()?;
        Ok(this)
    }

    pub fn resize(
        &mut self,
        width_points: f32,
        height_points: f32,
        display_scale: f32,
    ) -> Result<()> {
        ensure_main_thread()?;
        let size_points = validated_size(width_points, height_points, display_scale)?;
        if self.size_points == size_points && self.display_scale == display_scale {
            return Ok(());
        }

        self.size_points = size_points;
        self.display_scale = display_scale;
        self.recompute_metrics()?;
        self.resources
            .update_surface_size(self.physical_size())
            .map_err(|error| anyhow!(error))?;
        self.render()
    }

    pub fn render(&mut self) -> Result<()> {
        ensure_main_thread()?;
        let mut scene = Scene::new(
            self.display_scale,
            Config {
                gpu_power_preference: GPUPowerPreference::LowPower,
                ..Default::default()
            },
        );
        scene
            .draw_rect_without_hit_recording(RectF::new(Vector2F::zero(), self.size_points))
            .with_background(self.terminal_background());
        self.draw_terminal_frame(&mut scene, Instant::now());

        self.renderer
            .render(
                &scene,
                &self.resources,
                &|glyph_key, scale, subpixel_alignment, glyph_config, format| {
                    self.rasterizer.rasterize_glyph(
                        glyph_key.font_id,
                        glyph_key.font_size.into_inner(),
                        glyph_key.glyph_id,
                        scale,
                        subpixel_alignment,
                        glyph_config,
                        format,
                    )
                },
                &|glyph_key, scale, glyph_config| {
                    self.rasterizer.glyph_raster_bounds(
                        glyph_key.font_id,
                        glyph_key.font_size.into_inner(),
                        glyph_key.glyph_id,
                        scale,
                        glyph_config,
                    )
                },
                self.physical_size(),
                None,
                None,
            )
            .map_err(|error| anyhow!(error))
    }

    pub fn set_terminal_frame(&mut self, frame: TerminalFrame) -> Result<()> {
        if self.terminal_frame.cursor != frame.cursor || self.terminal_frame.cells != frame.cells {
            self.note_activity();
        }
        if self.terminal_frame.cursor != frame.cursor {
            self.retarget_cursor(frame.cursor, Instant::now());
        }
        self.terminal_frame = frame;
        self.render()
    }

    pub fn apply_settings(&mut self, settings: TerminalRenderSettings) -> Result<bool> {
        if self.settings == settings {
            return Ok(false);
        }
        let metrics_changed = self.settings.font_family != settings.font_family
            || self.settings.font_size != settings.font_size
            || self.settings.line_height_ratio != settings.line_height_ratio;
        let effects_disabled = !settings.effects_enabled;
        let cursor_motion_changed = self.settings.cursor_motion != settings.cursor_motion;
        let cursor_motion_disabled = effects_disabled
            || matches!(settings.cursor_motion, TerminalCursorMotion::Snap)
            || settings.reduced_motion;
        self.settings = settings;
        if metrics_changed {
            self.recompute_metrics()?;
        }
        if effects_disabled {
            self.effects.clear();
            self.pending_cursor_duration_scale = None;
        }
        if cursor_motion_disabled || cursor_motion_changed {
            self.cursor_motion = None;
        }
        self.note_activity();
        self.render()?;
        Ok(metrics_changed)
    }

    pub fn trigger_effect(&mut self, effect: TerminalVisualEffect) -> Result<()> {
        if !self.settings.effects_enabled {
            return Ok(());
        }
        self.effects.push(
            effect,
            self.settings.effect_duration_ms,
            self.settings.reduced_motion,
        );
        self.render()
    }

    pub fn note_activity(&mut self) {
        self.last_activity = Instant::now();
        self.cursor_visible = true;
    }

    pub fn prepare_backward_cursor_motion(&mut self) {
        self.pending_cursor_duration_scale = Some(0.25);
    }

    pub fn advance_animations(&mut self) -> Result<bool> {
        let now = Instant::now();
        let effects_were_active = self.effects.is_active();
        let effects_changed = effects_were_active && self.effects.advance(now);
        let cursor_motion_active = self
            .cursor_motion
            .is_some_and(|motion| motion.is_active(now));
        if self
            .cursor_motion
            .is_some_and(|motion| !motion.is_active(now))
        {
            self.cursor_motion = None;
        }
        let cursor_visible = self.desired_cursor_visibility(now);
        let cursor_changed = cursor_visible != self.cursor_visible;
        self.cursor_visible = cursor_visible;
        if effects_changed || cursor_changed || cursor_motion_active {
            self.render()?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn grid_size(&self) -> (u16, u16) {
        let columns = ((self.size_points.x() - HORIZONTAL_PADDING * 2.0) / self.metrics.cell_width)
            .floor()
            .max(1.0) as u16;
        let rows = ((self.size_points.y() - VERTICAL_PADDING * 2.0) / self.metrics.line_height)
            .floor()
            .max(1.0) as u16;
        (rows, columns)
    }

    pub fn grid_position(&self, x_points: f32, y_points: f32) -> (u16, u16) {
        let (rows, columns) = self.grid_size();
        let column = ((x_points - HORIZONTAL_PADDING) / self.metrics.cell_width)
            .floor()
            .clamp(0.0, f32::from(columns.saturating_sub(1))) as u16;
        let row = ((y_points - VERTICAL_PADDING) / self.metrics.line_height)
            .floor()
            .clamp(0.0, f32::from(rows.saturating_sub(1))) as u16;
        (row, column)
    }

    fn physical_size(&self) -> Vector2F {
        self.size_points * self.display_scale
    }

    fn glyph_for_char(&self, character: char) -> Option<(FontId, u32)> {
        let primary = primary_font_index(self.settings.font_family, self.fonts.len());
        self.fonts
            .get(primary)
            .and_then(|font| {
                font.font
                    .glyph_for_char(character)
                    .map(|glyph_id| (font.id, glyph_id))
            })
            .or_else(|| {
                self.fonts.iter().enumerate().find_map(|(index, font)| {
                    (index != primary)
                        .then(|| {
                            font.font
                                .glyph_for_char(character)
                                .map(|glyph_id| (font.id, glyph_id))
                        })
                        .flatten()
                })
            })
    }

    fn recompute_metrics(&mut self) -> Result<()> {
        let primary = primary_font_index(self.settings.font_family, self.fonts.len());
        self.metrics = TerminalMetrics::new(
            &self.fonts[primary].font,
            self.display_scale,
            self.settings.font_size,
            self.settings.line_height_ratio,
        )?;
        Ok(())
    }

    fn desired_cursor_visibility(&self, now: Instant) -> bool {
        if !self.settings.cursor_blink || self.terminal_frame.cursor.is_none() {
            return true;
        }
        let idle = now.duration_since(self.last_activity);
        if idle < CURSOR_IDLE_DELAY {
            return true;
        }
        let blink_elapsed = idle - CURSOR_IDLE_DELAY;
        (blink_elapsed.as_millis() / CURSOR_BLINK_HALF_PERIOD.as_millis()) % 2 == 0
    }
}

fn load_system_fallback_fonts(font_size: f32) -> Vec<Arc<Font>> {
    [
        ("AppleColorEmoji", '🐲'),
        ("PingFangSC-Regular", '中'),
        ("HiraginoSans-W3", '中'),
    ]
    .into_iter()
    .filter_map(|(font_name, sample)| {
        let native = core_text::font::new_from_name(font_name, f64::from(font_size)).ok()?;
        // SAFETY: `native` is a live CoreText font object and font-kit retains its native handle.
        let font = unsafe { Font::from_native_font(native) };
        font.glyph_for_char(sample).map(|_| Arc::new(font))
    })
    .collect()
}

fn pixel_ceil(value: f32, display_scale: f32) -> f32 {
    (value * display_scale).ceil() / display_scale
}

fn pixel_round(value: f32, display_scale: f32) -> f32 {
    (value * display_scale).round() / display_scale
}

fn color([r, g, b, a]: [u8; 4]) -> ColorU {
    ColorU::new(r, g, b, a)
}

fn primary_font_index(family: TerminalFontFamily, font_count: usize) -> usize {
    match family {
        TerminalFontFamily::JetBrainsMono => 0,
        TerminalFontFamily::MapleMonoNF if font_count > 1 => 1,
        TerminalFontFamily::MapleMonoNF => 0,
    }
}

fn validated_size(width_points: f32, height_points: f32, display_scale: f32) -> Result<Vector2F> {
    if !width_points.is_finite()
        || !height_points.is_finite()
        || !display_scale.is_finite()
        || width_points <= 0.0
        || height_points <= 0.0
        || display_scale <= 0.0
    {
        bail!("invalid UIKit render surface size");
    }
    Ok(Vector2F::new(width_points, height_points))
}

fn ensure_main_thread() -> Result<()> {
    // SAFETY: `pthread_main_np` takes no arguments and only reports whether the caller is the
    // process main thread.
    if unsafe { pthread_main_np() } == 0 {
        bail!("UIKit rendering must run on the main thread");
    }
    Ok(())
}
