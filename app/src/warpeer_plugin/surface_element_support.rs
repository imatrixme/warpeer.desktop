use pathfinder_geometry::vector::Vector2F;
use warpeer_plugin_protocol::{SurfaceBounds, SurfaceContainerMetrics};

pub(crate) const PLUGIN_WINDOW_TITLEBAR_HEIGHT: f32 = 24.0;
const PLUGIN_WINDOW_BORDER_WIDTH: f32 = 1.0;

pub(crate) const fn plugin_window_container_metrics() -> SurfaceContainerMetrics {
    SurfaceContainerMetrics {
        border_width: PLUGIN_WINDOW_BORDER_WIDTH,
        titlebar_height: PLUGIN_WINDOW_TITLEBAR_HEIGHT,
    }
}

pub(crate) fn container_bounds(
    content: SurfaceBounds,
    container: SurfaceContainerMetrics,
) -> SurfaceBounds {
    SurfaceBounds {
        width: content.width + container.border_width * 2.0,
        height: content.height + container.titlebar_height + container.border_width * 2.0,
        ..content
    }
}

pub(super) fn rgba(color: pathfinder_color::ColorU) -> [u8; 4] {
    [color.r, color.g, color.b, color.a]
}

pub(super) fn color(rgba: [u8; 4]) -> pathfinder_color::ColorU {
    pathfinder_color::ColorU::new(rgba[0], rgba[1], rgba[2], rgba[3])
}

pub(super) fn content_size(size: Vector2F, container: SurfaceContainerMetrics) -> Vector2F {
    Vector2F::new(
        (size.x() - container.border_width * 2.0).max(1.0),
        (size.y() - container.titlebar_height - container.border_width * 2.0).max(1.0),
    )
}

pub(super) fn modifier_bits(keystroke: &warpui::keymap::Keystroke) -> u16 {
    u16::from(keystroke.ctrl)
        | (u16::from(keystroke.alt) << 1)
        | (u16::from(keystroke.shift) << 2)
        | (u16::from(keystroke.cmd) << 3)
        | (u16::from(keystroke.meta) << 4)
}

pub(super) fn rgba_from_bgra(
    bytes: &[u8],
    stride_bytes: u32,
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    let row_bytes = width.checked_mul(4)? as usize;
    let stride = stride_bytes as usize;
    if stride < row_bytes || bytes.len() < stride.checked_mul(height as usize)? {
        return None;
    }
    let mut rgba = Vec::with_capacity(row_bytes.checked_mul(height as usize)?);
    for row in bytes.chunks_exact(stride).take(height as usize) {
        for pixel in row[..row_bytes].chunks_exact(4) {
            rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }
    Some(rgba)
}

#[cfg(target_os = "macos")]
pub(super) fn system_reduce_motion_enabled() -> bool {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let enabled: bool = msg_send![workspace, accessibilityDisplayShouldReduceMotion];
        enabled
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn system_reduce_motion_enabled() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgra_conversion_honors_stride_and_channel_order() {
        let rgba = rgba_from_bgra(&[1, 2, 3, 4, 0, 0, 0, 0], 8, 1, 1).unwrap();
        assert_eq!(rgba, [3, 2, 1, 4]);
        assert!(rgba_from_bgra(&[0; 3], 4, 1, 1).is_none());
    }

    #[test]
    fn modifier_encoding_is_stable() {
        let stroke = warpui::keymap::Keystroke {
            ctrl: true,
            alt: false,
            shift: true,
            cmd: true,
            meta: false,
            key: "a".into(),
        };
        assert_eq!(modifier_bits(&stroke), 13);
    }
}
