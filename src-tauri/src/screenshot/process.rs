use crate::error::{AgentError, AgentResult};
use image::imageops::FilterType;
use image::DynamicImage;
use mozjpeg_rs::{Encoder, Preset, Subsampling};

use super::constants::{DEFAULT_JPEG_QUALITY, MAX_SCREENSHOT_LONG_EDGE_PX};

pub(crate) fn process_capture_bytes(
    png_bytes: &[u8],
    blur_enabled: bool,
    jpeg_quality: u8,
) -> AgentResult<(i32, i32, Vec<u8>)> {
    let image = image::load_from_memory(png_bytes)
        .map_err(|err| AgentError::Other(format!("failed to decode screenshot: {err}")))?;
    let processed = if blur_enabled {
        image.blur(super::constants::BLUR_SIGMA)
    } else {
        image
    };
    let resized = resize_for_storage(processed);
    let width = resized.width() as i32;
    let height = resized.height() as i32;
    let jpeg_bytes = encode_jpeg(&resized, jpeg_quality)?;
    Ok((width, height, jpeg_bytes))
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

fn encode_jpeg(image: &DynamicImage, quality: u8) -> AgentResult<Vec<u8>> {
    let rgb = image.to_rgb8();
    let jpeg_bytes = Encoder::new(Preset::ProgressiveBalanced)
        .quality(quality.clamp(1, 100))
        .subsampling(Subsampling::S444)
        .encode_rgb(rgb.as_raw(), rgb.width(), rgb.height())
        .map_err(|err| AgentError::Other(format!("failed to encode jpeg: {err}")))?;
    Ok(jpeg_bytes)
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
    use image::{ImageBuffer, ImageFormat, Rgba};

    fn sample_png_bytes(width: u32, height: u32) -> Vec<u8> {
        let image = ImageBuffer::from_fn(width, height, |x, y| {
            Rgba([
                ((x * 3) % 256) as u8,
                ((y * 5) % 256) as u8,
                ((x + y) % 256) as u8,
                255,
            ])
        });
        let mut bytes = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn produces_valid_jpeg() {
        let (_, _, jpeg) = process_capture_bytes(&sample_png_bytes(128, 128), false, 80).unwrap();
        assert!(jpeg.starts_with(&[0xFF, 0xD8, 0xFF]));
    }

    #[test]
    fn blur_changes_output_when_enabled() {
        let png = sample_png_bytes(128, 128);
        let (_, _, without_blur) = process_capture_bytes(&png, false, 80).unwrap();
        let (_, _, with_blur) = process_capture_bytes(&png, true, 80).unwrap();
        assert_ne!(without_blur, with_blur);
    }

    #[test]
    fn jpeg_is_smaller_than_png_for_photo_like_fixture() {
        let png = sample_png_bytes(128, 128);
        let (_, _, jpeg) = process_capture_bytes(&png, false, 80).unwrap();
        assert!(jpeg.len() < png.len());
    }

    #[test]
    fn downscales_large_captures_before_encoding() {
        let png = sample_png_bytes(3840, 2160);
        let (width, height, jpeg) = process_capture_bytes(&png, false, 80).unwrap();
        assert_eq!(width, 1920);
        assert_eq!(height, 1080);
        assert!(jpeg.starts_with(&[0xFF, 0xD8, 0xFF]));
    }

    #[test]
    fn large_capture_is_much_smaller_than_full_resolution_png() {
        let png = sample_png_bytes(2560, 1440);
        let (_, _, jpeg) = process_capture_bytes(&png, false, 80).unwrap();
        assert!(jpeg.len() < png.len() / 4);
    }
}
