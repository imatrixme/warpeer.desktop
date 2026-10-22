use std::fs;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use image::{DynamicImage, GenericImageView as _, ImageFormat};
use url::Url;
use warpeer_plugin_runtime::{FetchPolicy, fetch_exact_https};

pub(super) struct MaterializedImage {
    pub rgba: Vec<u8>,
    pub width: u16,
    pub height: u16,
    pub original_width: u32,
    pub original_height: u32,
}

pub(super) struct LoadedImageResource {
    pub image: MaterializedImage,
    pub media_type: &'static str,
}

pub(super) fn load_image_resource(
    source_url: &str,
    max_bytes: u64,
    max_width: u16,
    max_height: u16,
) -> Result<LoadedImageResource> {
    let url = Url::parse(source_url).context("parse bridge image source URL")?;
    let bytes = match url.scheme() {
        "file" => {
            let path = url
                .to_file_path()
                .map_err(|_| anyhow::anyhow!("invalid bridge image file URL"))?;
            let metadata = fs::metadata(&path).context("stat bridge image")?;
            if metadata.len() > max_bytes {
                bail!(
                    "bridge image is {} bytes; maximum is {max_bytes}",
                    metadata.len()
                );
            }
            fs::read(path).context("read bridge image")?
        }
        "https" => {
            fetch_exact_https(
                &url,
                FetchPolicy {
                    max_bytes,
                    timeout: Duration::from_secs(8),
                },
            )
            .context("fetch bridge image")?
            .bytes
        }
        scheme => bail!("unsupported bridge image source scheme: {scheme}"),
    };
    let format = image::guess_format(&bytes).context("identify bridge image")?;
    let media_type = match format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        _ => bail!("unsupported bridge image format: {format:?}"),
    };
    let image = decode_image_resource(&bytes, max_width, max_height)?;
    Ok(LoadedImageResource { image, media_type })
}

pub(super) fn decode_image_resource(
    bytes: &[u8],
    max_width: u16,
    max_height: u16,
) -> Result<MaterializedImage> {
    let format = image::guess_format(bytes).context("identify bridge image")?;
    if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg) {
        bail!("unsupported bridge image format: {format:?}");
    }
    let decoded =
        image::load_from_memory_with_format(bytes, format).context("decode bridge image")?;
    let (original_width, original_height) = decoded.dimensions();
    if original_width == 0 || original_height == 0 {
        bail!("bridge image is empty");
    }
    let decoded = contain_image(decoded, u32::from(max_width), u32::from(max_height));
    let (width, height) = decoded.dimensions();
    Ok(MaterializedImage {
        rgba: decoded.to_rgba8().into_raw(),
        width: u16::try_from(width).context("bridge image width exceeds u16")?,
        height: u16::try_from(height).context("bridge image height exceeds u16")?,
        original_width,
        original_height,
    })
}

fn contain_image(image: DynamicImage, max_width: u32, max_height: u32) -> DynamicImage {
    let (width, height) = image.dimensions();
    if width <= max_width && height <= max_height {
        image
    } else {
        image.resize(
            max_width.max(1),
            max_height.max(1),
            image::imageops::FilterType::Lanczos3,
        )
    }
}
