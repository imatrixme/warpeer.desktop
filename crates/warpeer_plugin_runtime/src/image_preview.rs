use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use image::{DynamicImage, GenericImageView, ImageFormat};
use serde::{Deserialize, Serialize};
use url::Url;
use warpeer_plugin_protocol::{ContextId, Revision, TerminalHoverContext};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct ImagePreviewConfig {
    pub hover_delay_ms: u64,
    pub max_width: u32,
    pub max_height: u32,
    pub remote_https_enabled: bool,
    pub cache_bytes: u64,
    pub max_resource_bytes: u64,
}

impl Default for ImagePreviewConfig {
    fn default() -> Self {
        Self {
            hover_delay_ms: 1500,
            max_width: 560,
            max_height: 420,
            remote_https_enabled: true,
            cache_bytes: 32 * 1024 * 1024,
            max_resource_bytes: 16 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ImageTarget {
    Local(PathBuf),
    Remote(Url),
}

impl ImageTarget {
    pub fn display_name(&self) -> String {
        match self {
            Self::Local(path) => path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Image")
                .to_owned(),
            Self::Remote(url) => url
                .path_segments()
                .and_then(Iterator::last)
                .filter(|name| !name.is_empty())
                .unwrap_or("Image")
                .to_owned(),
        }
    }
}

pub fn recognize_target(
    context: &TerminalHoverContext,
    config: &ImagePreviewConfig,
) -> Option<ImageTarget> {
    let candidate = context
        .resolved_resource
        .as_deref()
        .unwrap_or(&context.resource_text);
    if let Ok(url) = Url::parse(candidate) {
        return (config.remote_https_enabled
            && url.scheme() == "https"
            && supported_extension(url.path()))
        .then_some(ImageTarget::Remote(url));
    }
    if !supported_extension(candidate) {
        return None;
    }
    let path = Path::new(candidate);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        Path::new(context.working_directory.as_deref()?).join(path)
    };
    Some(ImageTarget::Local(resolved))
}

fn supported_extension(value: &str) -> bool {
    matches!(
        Path::new(value)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("png" | "jpg" | "jpeg")
    )
}

#[derive(Clone, Debug)]
pub struct DecodedPreview {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub original_width: u32,
    pub original_height: u32,
    pub format: ImageFormat,
}

impl DecodedPreview {
    pub fn byte_len(&self) -> u64 {
        self.rgba.len() as u64
    }
}

pub fn decode_preview(
    bytes: &[u8],
    config: &ImagePreviewConfig,
) -> Result<DecodedPreview, ImagePreviewError> {
    if bytes.len() as u64 > config.max_resource_bytes {
        return Err(ImagePreviewError::ResourceTooLarge(bytes.len() as u64));
    }
    let format = image::guess_format(bytes).map_err(ImagePreviewError::Decode)?;
    if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg) {
        return Err(ImagePreviewError::UnsupportedFormat(format));
    }
    let decoded =
        image::load_from_memory_with_format(bytes, format).map_err(ImagePreviewError::Decode)?;
    let (original_width, original_height) = decoded.dimensions();
    if original_width == 0 || original_height == 0 {
        return Err(ImagePreviewError::EmptyImage);
    }
    let resized = contain(decoded, config.max_width, config.max_height);
    let (width, height) = resized.dimensions();
    Ok(DecodedPreview {
        rgba: resized.to_rgba8().into_raw(),
        width,
        height,
        original_width,
        original_height,
        format,
    })
}

fn contain(image: DynamicImage, max_width: u32, max_height: u32) -> DynamicImage {
    let (width, height) = image.dimensions();
    if width <= max_width && height <= max_height {
        return image;
    }
    image.resize(max_width, max_height, image::imageops::FilterType::Lanczos3)
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PreviewCacheKey {
    pub target: ImageTarget,
    pub resource_revision: String,
}

pub struct PreviewCache {
    entries: HashMap<PreviewCacheKey, DecodedPreview>,
    order: VecDeque<PreviewCacheKey>,
    bytes: u64,
    budget: u64,
}

impl PreviewCache {
    pub fn new(budget: u64) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            budget,
        }
    }

    pub fn get(&mut self, key: &PreviewCacheKey) -> Option<&DecodedPreview> {
        if self.entries.contains_key(key) {
            self.order.retain(|known| known != key);
            self.order.push_back(key.clone());
        }
        self.entries.get(key)
    }

    pub fn insert(&mut self, key: PreviewCacheKey, preview: DecodedPreview) {
        if preview.byte_len() > self.budget {
            return;
        }
        if let Some(previous) = self.entries.remove(&key) {
            self.bytes = self.bytes.saturating_sub(previous.byte_len());
            self.order.retain(|known| known != &key);
        }
        while self.bytes.saturating_add(preview.byte_len()) > self.budget {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(removed.byte_len());
            }
        }
        self.bytes = self.bytes.saturating_add(preview.byte_len());
        self.order.push_back(key.clone());
        self.entries.insert(key, preview);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.bytes = 0;
    }

    pub fn set_budget(&mut self, budget: u64) {
        self.budget = budget;
        while self.bytes > self.budget {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(removed.byte_len());
            }
        }
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActiveHover {
    pub context_id: ContextId,
    pub revision: Revision,
}

impl ActiveHover {
    pub fn accepts(self, context_id: ContextId, revision: Revision) -> bool {
        self.context_id == context_id && self.revision == revision
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ImagePreviewError {
    #[error("image resource has {0} bytes and exceeds the configured limit")]
    ResourceTooLarge(u64),
    #[error("image format is not supported: {0:?}")]
    UnsupportedFormat(ImageFormat),
    #[error("image is empty")]
    EmptyImage,
    #[error("image could not be decoded: {0}")]
    Decode(#[source] image::ImageError),
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn context(resource: &str) -> TerminalHoverContext {
        TerminalHoverContext {
            context_id: ContextId::new(),
            revision: Revision(1),
            terminal_id: "terminal".into(),
            pane_id: "pane".into(),
            workspace_id: None,
            working_directory: Some("/tmp".into()),
            resource_text: resource.into(),
            resolved_resource: None,
            explicit_hyperlink: false,
            explicit_preview: false,
            anchor: [0.0; 4],
            viewport: [0.0; 4],
        }
    }

    fn encoded(format: ImageFormat, width: u32, height: u32) -> Vec<u8> {
        let mut output = Cursor::new(Vec::new());
        DynamicImage::new_rgb8(width, height)
            .write_to(&mut output, format)
            .unwrap();
        output.into_inner()
    }

    #[test]
    fn recognizes_local_and_remote_supported_images() {
        assert_eq!(
            recognize_target(&context("sample.png"), &ImagePreviewConfig::default()),
            Some(ImageTarget::Local("/tmp/sample.png".into()))
        );
        assert!(matches!(
            recognize_target(
                &context("https://example.com/sample.jpeg?revision=2"),
                &ImagePreviewConfig::default()
            ),
            Some(ImageTarget::Remote(_))
        ));
        assert!(recognize_target(&context("notes.txt"), &ImagePreviewConfig::default()).is_none());
    }

    #[test]
    fn decodes_png_and_jpeg_and_preserves_aspect_ratio() {
        let config = ImagePreviewConfig {
            max_width: 20,
            max_height: 20,
            ..ImagePreviewConfig::default()
        };
        let png = decode_preview(&encoded(ImageFormat::Png, 80, 40), &config).unwrap();
        assert_eq!((png.width, png.height), (20, 10));
        assert_eq!((png.original_width, png.original_height), (80, 40));
        assert_eq!(png.format, ImageFormat::Png);
        let jpeg = decode_preview(&encoded(ImageFormat::Jpeg, 10, 8), &config).unwrap();
        assert_eq!((jpeg.width, jpeg.height), (10, 8));
        assert_eq!(jpeg.format, ImageFormat::Jpeg);
        assert!(decode_preview(b"not an image", &config).is_err());
    }

    #[test]
    fn cache_evicts_old_entries_without_spilling() {
        let decoded = decode_preview(
            &encoded(ImageFormat::Png, 4, 2),
            &ImagePreviewConfig::default(),
        )
        .unwrap();
        let mut cache = PreviewCache::new(decoded.byte_len());
        let first = PreviewCacheKey {
            target: ImageTarget::Local("/tmp/one.png".into()),
            resource_revision: "one".into(),
        };
        let second = PreviewCacheKey {
            target: ImageTarget::Local("/tmp/two.png".into()),
            resource_revision: "two".into(),
        };
        cache.insert(first.clone(), decoded.clone());
        cache.insert(second.clone(), decoded);
        assert!(cache.get(&first).is_none());
        assert!(cache.get(&second).is_some());
        assert!(cache.bytes() > 0);
        cache.clear();
        assert_eq!(cache.bytes(), 0);
    }
}
