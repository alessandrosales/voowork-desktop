use crate::error::{AgentError, AgentResult};
use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage};
use webp::Encoder as WebpEncoder;

use super::constants::{DEFAULT_JPEG_QUALITY, MAX_SCREENSHOT_LONG_EDGE_PX};

pub(crate) fn process_raw_rgba(
    rgba_bytes: &[u8],
    width: i32,
    height: i32,
    blur_enabled: bool,
    quality: u8,
) -> AgentResult<(i32, i32, Vec<u8>)> {
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(width as u32, height as u32, rgba_bytes.to_vec())
            .ok_or_else(|| AgentError::Other("invalid RGBA dimensions".into()))?,
    );
    let processed = if blur_enabled {
        image.blur(super::constants::BLUR_SIGMA)
    } else {
        image
    };
    let resized = resize_for_storage(processed);
    let width = resized.width() as i32;
    let height = resized.height() as i32;
    let webp_bytes = encode_webp(&resized, quality)?;
    Ok((width, height, webp_bytes))
}

fn resize_for_storage(image: DynamicImage) -> DynamicImage {
    let (width, height) = (image.width(), image.height());
    let long_edge = width.max(height);
    if long_edge <= MAX_SCREENSHOT_LONG_EDGE_PX {
        return image;
    }

    let scale = MAX_SCREENSHOT_LONG_EDGE_PX as f32 / long_edge as f32;
    let new_width = ((width as f32 * scale).round() as u32).max(1);
    let new_height = ((height as f32 * scale).round() as u32).max(1);
    image.resize(new_width, new_height, FilterType::Lanczos3)
}

fn encode_webp(image: &DynamicImage, quality: u8) -> AgentResult<Vec<u8>> {
    let rgb = image.to_rgb8();
    let encoder = WebpEncoder::from_rgb(rgb.as_raw(), rgb.width(), rgb.height());
    let encoded = encoder.encode(quality.clamp(1, 100) as f32);
    Ok(encoded.to_vec())
}

pub fn normalize_jpeg_quality(quality: u8) -> u8 {
    if (1..=100).contains(&quality) {
        quality
    } else {
        DEFAULT_JPEG_QUALITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;

    fn sample_rgba_bytes(width: u32, height: u32) -> (Vec<u8>, i32, i32) {
        let image = RgbaImage::from_fn(width, height, |x, y| {
            image::Rgba([
                ((x * 3) % 256) as u8,
                ((y * 5) % 256) as u8,
                ((x + y) % 256) as u8,
                255,
            ])
        });
        (image.into_raw(), width as i32, height as i32)
    }

    #[test]
    fn produces_valid_webp() {
        let (rgba, w, h) = sample_rgba_bytes(128, 128);
        let (_, _, webp) = process_raw_rgba(&rgba, w, h, false, 80).unwrap();
        assert!(webp.starts_with(b"RIFF"), "expected WEBP RIFF header");
        assert!(
            webp.windows(4).any(|w| w == b"WEBP"),
            "expected WEBP chunk header"
        );
    }

    #[test]
    fn blur_changes_output_when_enabled() {
        let (rgba, w, h) = sample_rgba_bytes(128, 128);
        let (_, _, without_blur) = process_raw_rgba(&rgba, w, h, false, 80).unwrap();
        let (_, _, with_blur) = process_raw_rgba(&rgba, w, h, true, 80).unwrap();
        assert_ne!(without_blur, with_blur);
    }

    #[test]
    fn downscales_large_captures_before_encoding() {
        let (rgba, w, h) = sample_rgba_bytes(3840, 2160);
        let (width, height, webp) = process_raw_rgba(&rgba, w, h, false, 80).unwrap();
        assert_eq!(width, 1920);
        assert_eq!(height, 1080);
        assert!(webp.starts_with(b"RIFF"));
    }
}
