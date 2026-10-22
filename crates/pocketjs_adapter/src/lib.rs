//! PocketJS dependency boundary for Warpeer surface plugins.

use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr::NonNull;
use std::sync::Mutex;

use pocket_apple::{
    PocketApple, PocketAppleFrame, pocket_apple_create, pocket_apple_destroy,
    pocket_apple_eval_bundle, pocket_apple_frame, pocket_apple_free_texture,
    pocket_apple_last_error, pocket_apple_load_pak, pocket_apple_post_event, pocket_apple_render,
    pocket_apple_resize, pocket_apple_set_effect_callback, pocket_apple_set_identity,
    pocket_apple_set_tick_rate, pocket_apple_upload_rgba,
};
use warpeer_plugin_protocol::Revision;

pub const PINNED_POCKETJS_REVISION: &str = "3f8396fb11ceb72ff80703e8a440b3a939bde7bc";
pub const WARPEER_POCKET_HOST_ID: &str = "warpeer-plugin";
pub const WARPEER_POCKET_HOST_ABI: u32 = 1;

pub struct PocketRuntime {
    handle: NonNull<PocketApple>,
    frame_revision: Revision,
    effects: Box<Mutex<Vec<String>>>,
}

extern "C" fn collect_effect(line: *const c_char, context: *mut c_void) {
    if line.is_null() || context.is_null() {
        return;
    }
    let queue = unsafe { &*(context as *const Mutex<Vec<String>>) };
    let line = unsafe { CStr::from_ptr(line) }
        .to_string_lossy()
        .into_owned();
    if let Ok(mut effects) = queue.lock() {
        effects.push(line);
    }
}

impl PocketRuntime {
    pub fn load(
        bundle: &[u8],
        pak: &[u8],
        logical_size: (u32, u32),
        density: u32,
    ) -> Result<Self, PocketError> {
        let handle = NonNull::new(pocket_apple_create(density, logical_size.0, logical_size.1))
            .ok_or_else(last_error)?;
        let mut runtime = Self {
            handle,
            frame_revision: Revision(0),
            effects: Box::new(Mutex::new(Vec::new())),
        };
        runtime.check(pocket_apple_set_identity(
            runtime.handle.as_ptr(),
            CString::new(WARPEER_POCKET_HOST_ID).unwrap().as_ptr(),
            WARPEER_POCKET_HOST_ABI,
        ))?;
        runtime.check(pocket_apple_set_tick_rate(runtime.handle.as_ptr(), 60))?;
        runtime.check(pocket_apple_load_pak(
            runtime.handle.as_ptr(),
            pak.as_ptr(),
            pak.len(),
        ))?;
        let label = CString::new("warpeer-plugin").unwrap();
        runtime.check(pocket_apple_eval_bundle(
            runtime.handle.as_ptr(),
            bundle.as_ptr(),
            bundle.len(),
            label.as_ptr(),
        ))?;
        let effect_context = (&*runtime.effects as *const Mutex<Vec<String>>)
            .cast_mut()
            .cast();
        runtime.check(pocket_apple_set_effect_callback(
            runtime.handle.as_ptr(),
            Some(collect_effect),
            effect_context,
        ))?;
        Ok(runtime)
    }

    pub fn post_event(&mut self, event: &str) -> Result<(), PocketError> {
        let event = CString::new(event).map_err(|_| PocketError::InteriorNul)?;
        self.check(pocket_apple_post_event(
            self.handle.as_ptr(),
            event.as_ptr(),
        ))
    }

    pub fn tick(&mut self, touches: &[u32]) -> Result<(), PocketError> {
        self.check(pocket_apple_frame(
            self.handle.as_ptr(),
            0,
            0x8080,
            touches.as_ptr(),
            touches.len(),
        ))
    }

    pub fn upload_rgba(
        &mut self,
        pixels: &[u8],
        width: u16,
        height: u16,
    ) -> Result<PocketTexture, PocketError> {
        let texture = pocket_apple_upload_rgba(
            self.handle.as_ptr(),
            pixels.as_ptr(),
            pixels.len(),
            width,
            height,
        );
        if texture < 0 {
            Err(last_error())
        } else {
            Ok(PocketTexture(texture))
        }
    }

    pub fn register_image_url(
        &mut self,
        url: &str,
        pixels: &[u8],
        width: u16,
        height: u16,
    ) -> Result<PocketTexture, PocketError> {
        let texture = self.upload_rgba(pixels, width, height)?;
        let url =
            serde_json::to_string(url).map_err(|error| PocketError::Runtime(error.to_string()))?;
        let script = format!("globalThis.__warpeerRegisterImage?.({url}, {});", texture.0);
        if let Err(error) = self.eval_script("warpeer-image-resource", &script) {
            let _ = self.free_texture(texture);
            return Err(error);
        }
        Ok(texture)
    }

    pub fn free_texture(&mut self, texture: PocketTexture) -> Result<(), PocketError> {
        self.check(pocket_apple_free_texture(self.handle.as_ptr(), texture.0))
    }

    pub fn resize(&mut self, logical_size: (u32, u32)) -> Result<(), PocketError> {
        self.check(pocket_apple_resize(
            self.handle.as_ptr(),
            logical_size.0,
            logical_size.1,
        ))
    }

    fn eval_script(&mut self, label: &str, source: &str) -> Result<(), PocketError> {
        let label = CString::new(label).map_err(|_| PocketError::InteriorNul)?;
        self.check(pocket_apple_eval_bundle(
            self.handle.as_ptr(),
            source.as_ptr(),
            source.len(),
            label.as_ptr(),
        ))
    }

    pub fn drain_effects(&self) -> Vec<String> {
        self.effects
            .lock()
            .map(|mut effects| effects.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn render(&mut self) -> Result<Option<OwnedFrame>, PocketError> {
        let mut frame = PocketAppleFrame {
            pixels: std::ptr::null(),
            width_px: 0,
            height_px: 0,
            stride_bytes: 0,
            regions: [[0; 4]; pocket_apple::POCKET_APPLE_MAX_DAMAGE_REGIONS],
            region_count: 0,
            full_redraw: 0,
        };
        self.check(pocket_apple_render(self.handle.as_ptr(), &mut frame))?;
        if frame.region_count == 0 {
            return Ok(None);
        }
        let length = frame.stride_bytes as usize * frame.height_px as usize;
        if frame.pixels.is_null() || length == 0 {
            return Err(PocketError::InvalidFrame);
        }
        let pixels = unsafe { std::slice::from_raw_parts(frame.pixels, length) }.to_vec();
        self.frame_revision = self.frame_revision.next();
        Ok(Some(OwnedFrame {
            pixels,
            width_px: frame.width_px,
            height_px: frame.height_px,
            stride_bytes: frame.stride_bytes,
            dirty_regions: frame.regions[..frame.region_count as usize].to_vec(),
            full_redraw: frame.full_redraw != 0,
            revision: self.frame_revision,
        }))
    }

    fn check(&mut self, status: i32) -> Result<(), PocketError> {
        if status == 0 {
            Ok(())
        } else {
            Err(last_error())
        }
    }
}

impl Drop for PocketRuntime {
    fn drop(&mut self) {
        pocket_apple_destroy(self.handle.as_ptr());
    }
}

pub struct OwnedFrame {
    pub pixels: Vec<u8>,
    pub width_px: u32,
    pub height_px: u32,
    pub stride_bytes: u32,
    pub dirty_regions: Vec<[i32; 4]>,
    pub full_redraw: bool,
    pub revision: Revision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PocketTexture(pub i32);

fn last_error() -> PocketError {
    let pointer = pocket_apple_last_error();
    if pointer.is_null() {
        return PocketError::Runtime("PocketJS runtime failed".into());
    }
    let message = unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned();
    PocketError::Runtime(message)
}

#[derive(Debug, thiserror::Error)]
pub enum PocketError {
    #[error("PocketJS runtime failed: {0}")]
    Runtime(String),
    #[error("PocketJS event contains an interior NUL")]
    InteriorNul,
    #[error("PocketJS returned an invalid frame")]
    InvalidFrame,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pocket_runtime_is_lazy_and_renders_only_dirty_frames() {
        let bundle = b"globalThis.frame = function() {};";
        let mut runtime = PocketRuntime::load(bundle, b"pak", (32, 24), 1).unwrap();
        runtime.tick(&[]).unwrap();
        let frame = runtime.render().unwrap().unwrap();
        assert_eq!((frame.width_px, frame.height_px), (32, 24));
        runtime.tick(&[]).unwrap();
        assert!(runtime.render().unwrap().is_none());
    }

    #[test]
    fn external_rgba_textures_use_opaque_handles() {
        let bundle = b"globalThis.frame = function() {};";
        let mut runtime = PocketRuntime::load(bundle, b"pak", (32, 24), 1).unwrap();
        let texture = runtime.upload_rgba(&[255; 17 * 13 * 4], 17, 13).unwrap();
        assert!(texture.0 >= 0);
        runtime.free_texture(texture).unwrap();
        assert!(runtime.upload_rgba(&[0; 3], 4, 4).is_err());
    }

    #[test]
    fn resize_forces_a_new_frame_without_reloading_the_guest() {
        let bundle = b"globalThis.frame = function() {};";
        let mut runtime = PocketRuntime::load(bundle, b"pak", (32, 24), 1).unwrap();
        runtime.tick(&[]).unwrap();
        let _ = runtime.render().unwrap();
        runtime.resize((48, 40)).unwrap();
        let frame = runtime.render().unwrap().unwrap();
        assert_eq!((frame.width_px, frame.height_px), (48, 40));
    }
}
