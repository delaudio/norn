//! Shared bounded raster verification; no provider or filesystem access.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageSide {
    Old,
    New,
}

pub(crate) const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;

pub(crate) struct Raster {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
    pub width: u32,
    pub height: u32,
    pub sha256: String,
}
pub(crate) fn raster_format(path: &str) -> Result<(&'static str, image::ImageFormat), String> {
    let extension = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match extension.as_str() {
        "png" => Ok(("image/png", image::ImageFormat::Png)),
        "jpg" | "jpeg" => Ok(("image/jpeg", image::ImageFormat::Jpeg)),
        "gif" => Ok(("image/gif", image::ImageFormat::Gif)),
        "webp" => Ok(("image/webp", image::ImageFormat::WebP)),
        "bmp" => Ok(("image/bmp", image::ImageFormat::Bmp)),
        _ => Err("This image format is unavailable for immutable raster preview.".into()),
    }
}
pub(crate) fn validate_raster(path: &str, bytes: Vec<u8>) -> Result<Raster, String> {
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err("Image exceeds immutable preview limits.".into());
    }
    let (mime, expected) = raster_format(path)?;
    let mut reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|_| "Image content could not be verified.".to_string())?;
    if reader.format() != Some(expected) {
        return Err("Image content does not match its file format.".into());
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(40_000_000 * 4 + 16 * 1024 * 1024);
    reader.limits(limits);
    let (width, height) = reader
        .into_dimensions()
        .map_err(|_| "Image content could not be verified.".to_string())?;
    if width == 0
        || height == 0
        || width > 8192
        || height > 8192
        || u64::from(width) * u64::from(height) > 40_000_000
    {
        return Err("Image dimensions exceed immutable preview limits.".into());
    }
    Ok(Raster {
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        bytes,
        mime,
        width,
        height,
    })
}
