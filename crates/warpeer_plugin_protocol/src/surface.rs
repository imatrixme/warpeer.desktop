use serde::{Deserialize, Serialize};

use crate::{ContextId, Revision, SurfaceId};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SurfaceSlot {
    TerminalHover,
    PaneBackground,
    PaneOverlay,
    PaneHeader,
    PanePopover,
    PluginSettings,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SurfaceTarget {
    TerminalContext {
        context_id: ContextId,
        pane_id: String,
    },
    Pane {
        context_id: ContextId,
        pane_id: String,
    },
    PluginSettings {
        context_id: ContextId,
    },
}

impl SurfaceTarget {
    pub fn context_id(&self) -> ContextId {
        match self {
            Self::TerminalContext { context_id, .. }
            | Self::Pane { context_id, .. }
            | Self::PluginSettings { context_id } => *context_id,
        }
    }

    pub fn pane_id(&self) -> Option<&str> {
        match self {
            Self::TerminalContext { pane_id, .. } | Self::Pane { pane_id, .. } => Some(pane_id),
            Self::PluginSettings { .. } => None,
        }
    }

    pub fn accepts_slot(&self, slot: SurfaceSlot) -> bool {
        matches!(
            (self, slot),
            (Self::TerminalContext { .. }, SurfaceSlot::TerminalHover)
                | (
                    Self::Pane { .. },
                    SurfaceSlot::PaneBackground
                        | SurfaceSlot::PaneOverlay
                        | SurfaceSlot::PaneHeader
                        | SurfaceSlot::PanePopover
                )
                | (Self::PluginSettings { .. }, SurfaceSlot::PluginSettings)
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct SurfaceBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
}

impl SurfaceBounds {
    pub fn is_valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.scale.is_finite()
            && self.width > 0.0
            && self.height > 0.0
            && self.scale > 0.0
    }

    pub fn pixel_area(self) -> u64 {
        let width = (self.width.max(0.0) * self.scale.max(1.0)).ceil() as u64;
        let height = (self.height.max(0.0) * self.scale.max(1.0)).ceil() as u64;
        width.saturating_mul(height)
    }

    pub fn contains(self, point: SurfacePoint) -> bool {
        point.x >= self.x
            && point.y >= self.y
            && point.x < self.x + self.width
            && point.y < self.y + self.height
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct SurfacePoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct SurfaceContentSize {
    pub width: f32,
    pub height: f32,
}

impl SurfaceContentSize {
    pub fn is_valid(self) -> bool {
        self.width.is_finite() && self.height.is_finite() && self.width > 0.0 && self.height > 0.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SurfaceResizeEasing {
    Linear,
    EaseOut,
    EaseInOut,
    Spring,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct SurfaceResizeAnimation {
    pub duration_ms: u16,
    pub easing: SurfaceResizeEasing,
}

impl SurfaceResizeAnimation {
    pub const MAX_DURATION_MS: u16 = 600;

    pub fn is_valid(self) -> bool {
        self.duration_ms <= Self::MAX_DURATION_MS
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct SurfaceResizeRequest {
    pub surface_id: SurfaceId,
    pub revision: Revision,
    pub content_size: SurfaceContentSize,
    pub animation: Option<SurfaceResizeAnimation>,
}

impl SurfaceResizeRequest {
    pub fn is_valid(self) -> bool {
        self.content_size.is_valid() && self.animation.is_none_or(SurfaceResizeAnimation::is_valid)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SurfaceDescriptor {
    pub id: SurfaceId,
    pub slot: SurfaceSlot,
    pub target: SurfaceTarget,
    pub revision: Revision,
    pub bounds: SurfaceBounds,
    pub accessibility_label: String,
    pub accepts_pointer: bool,
    pub accepts_keyboard: bool,
}

impl SurfaceDescriptor {
    pub fn is_valid(&self) -> bool {
        self.bounds.is_valid()
            && self.target.accepts_slot(self.slot)
            && !self.accessibility_label.trim().is_empty()
            && self.accessibility_label.len() <= 256
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppearanceKind {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SurfacePalette {
    pub background: [u8; 4],
    pub surface: [u8; 4],
    pub text: [u8; 4],
    pub muted_text: [u8; 4],
    pub border: [u8; 4],
    pub accent: [u8; 4],
}

impl SurfacePalette {
    pub const fn dark() -> Self {
        Self {
            background: [15, 17, 21, 255],
            surface: [28, 31, 37, 255],
            text: [242, 244, 248, 255],
            muted_text: [161, 166, 176, 255],
            border: [69, 74, 84, 255],
            accent: [91, 156, 255, 255],
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SurfaceEnvironment {
    pub surface_id: SurfaceId,
    pub revision: Revision,
    pub bounds: SurfaceBounds,
    #[serde(default)]
    pub container: SurfaceContainerMetrics,
    pub appearance: AppearanceKind,
    pub palette: SurfacePalette,
    pub reduced_motion: bool,
    pub focused: bool,
    pub visible: bool,
    pub occluded: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct SurfaceContainerMetrics {
    pub border_width: f32,
    pub titlebar_height: f32,
}

impl Default for SurfaceContainerMetrics {
    fn default() -> Self {
        Self {
            border_width: 0.0,
            titlebar_height: 0.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SurfaceInput {
    PointerMove {
        point: SurfacePoint,
    },
    PointerDown {
        point: SurfacePoint,
        button: u8,
    },
    PointerUp {
        point: SurfacePoint,
        button: u8,
    },
    Scroll {
        delta_x: f32,
        delta_y: f32,
    },
    Key {
        key: String,
        modifiers: u16,
        down: bool,
    },
    Text {
        text: String,
    },
    Focus {
        focused: bool,
    },
    Dismiss,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "transport", rename_all = "kebab-case")]
pub enum FrameTransport {
    SharedBuffer {
        bytes: Vec<u8>,
        stride_bytes: u32,
    },
    #[cfg(target_os = "macos")]
    IoSurface {
        surface_id: u32,
        seed: u32,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SurfaceFrame {
    pub surface_id: SurfaceId,
    pub surface_revision: Revision,
    pub environment_revision: Revision,
    pub frame_revision: Revision,
    /// Guest raster dimensions in logical surface pixels. The host compositor
    /// maps this frame to the target display's backing scale.
    pub width_px: u32,
    pub height_px: u32,
    pub dirty_regions: Vec<[u32; 4]>,
    pub transport: FrameTransport,
}

impl SurfaceFrame {
    pub fn is_newer_than(&self, other: &Self) -> bool {
        self.surface_id == other.surface_id
            && self.surface_revision == other.surface_revision
            && self.environment_revision == other.environment_revision
            && self.frame_revision > other.frame_revision
    }
}
