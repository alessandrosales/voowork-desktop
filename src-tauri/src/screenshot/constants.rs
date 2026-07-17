pub const SCREENSHOT_FILE_EXTENSION: &str = "webp";
/// WebP quality (1–100, default 80). WebP is ~30% smaller than JPEG at same quality.
pub const DEFAULT_JPEG_QUALITY: u8 = 80;
/// Downscale captures above this long edge (px). 1920 keeps UI text readable on 4K.
pub const MAX_SCREENSHOT_LONG_EDGE_PX: u32 = 1920;
pub const BLUR_SIGMA: f32 = 10.0;
pub const SETTING_BLUR_ENABLED: &str = "screenshot_blur_enabled";
pub const SETTING_JPEG_QUALITY: &str = "screenshot_jpeg_quality";
