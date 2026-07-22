pub const SCREENSHOT_FILE_EXTENSION: &str = "webp";

pub const DEFAULT_JPEG_QUALITY: u8 = 80;

pub const MAX_SCREENSHOT_LONG_EDGE_PX: u32 = 1920;
pub const SETTING_BLUR_ENABLED: &str = "screenshot_blur_enabled";
pub const SETTING_JPEG_QUALITY: &str = "screenshot_jpeg_quality";
pub const RUNTIME_BLUR_POLICY_FILE: &str = "blur_policy.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlurLevel {
    None,
    Medium,
    Full,
}

impl BlurLevel {
    pub fn sigma(self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Medium => 5.0,
            Self::Full => 10.0,
        }
    }

    pub fn is_applied(self) -> bool {
        !matches!(self, Self::None)
    }
}
