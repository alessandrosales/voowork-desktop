use std::path::Path;

use crate::db::TIME_CATEGORY_INACTIVITY;
use crate::tracking_focus::ActiveWindowSample;

use super::constants::BlurLevel;

const BUNDLED_POLICY_JSON: &str = include_str!("../../resources/blur_policy.json");

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlurPolicyConfig {
    #[allow(dead_code)]
    version: u32,
    default_blur_level: BlurLevel,
    apps: Vec<AppBlurRule>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppBlurRule {
    name_pattern: String,
    blur_level: BlurLevel,
}

impl BlurPolicyConfig {
    pub fn load(runtime_path: Option<&Path>) -> Self {
        if let Some(path) = runtime_path {
            if let Ok(config) = Self::from_file(path) {
                log::info!("blur policy loaded from runtime override: {}", path.display());
                return config;
            }
            log::warn!(
                "failed to load runtime blur policy at {} — using bundled default",
                path.display()
            );
        }

        Self::from_json(BUNDLED_POLICY_JSON).unwrap_or_else(|err| {
            log::error!("failed to parse bundled blur policy: {err} — using hardcoded fallback");
            Self::fallback()
        })
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        Ok(Self::from_json(&contents)?)
    }

    pub fn blur_level_for(&self, app_name: &str) -> BlurLevel {
        let normalized = app_name.to_ascii_lowercase();
        self.apps
            .iter()
            .find(|rule| normalized.contains(&rule.name_pattern.to_ascii_lowercase()))
            .map(|rule| rule.blur_level)
            .unwrap_or(self.default_blur_level)
    }

    pub fn resolve_blur_level(
        &self,
        blur_override_full: bool,
        sample: Option<&ActiveWindowSample>,
        time_category: &str,
    ) -> BlurLevel {
        if blur_override_full {
            return BlurLevel::Full;
        }
        if time_category == TIME_CATEGORY_INACTIVITY {
            return BlurLevel::Full;
        }
        sample
            .map(|window| self.blur_level_for(&window.app_name))
            .unwrap_or(self.default_blur_level)
    }

    fn fallback() -> Self {
        Self {
            version: 1,
            default_blur_level: BlurLevel::Medium,
            apps: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(app_name: &str) -> ActiveWindowSample {
        ActiveWindowSample {
            app_name: app_name.into(),
            window_title: String::new(),
            process_path: None,
        }
    }

    #[test]
    fn bundled_policy_parses() {
        let policy = BlurPolicyConfig::load(None);
        assert_eq!(policy.blur_level_for("Cursor"), BlurLevel::None);
        assert_eq!(policy.blur_level_for("Slack"), BlurLevel::Full);
        assert_eq!(policy.blur_level_for("Google Chrome"), BlurLevel::Medium);
    }

    #[test]
    fn unknown_app_uses_default() {
        let policy = BlurPolicyConfig::load(None);
        assert_eq!(policy.blur_level_for("SomeUnknownApp"), BlurLevel::Medium);
    }

    #[test]
    fn resolve_blur_level_idle_overrides_dev_app() {
        let policy = BlurPolicyConfig::load(None);
        let window = sample("Cursor");
        assert_eq!(
            policy.resolve_blur_level(false, Some(&window), TIME_CATEGORY_INACTIVITY),
            BlurLevel::Full
        );
    }

    #[test]
    fn resolve_blur_level_global_override_forces_full() {
        let policy = BlurPolicyConfig::load(None);
        let window = sample("Cursor");
        assert_eq!(
            policy.resolve_blur_level(true, Some(&window), "active"),
            BlurLevel::Full
        );
    }

    #[test]
    fn resolve_blur_level_communication_app() {
        let policy = BlurPolicyConfig::load(None);
        let window = sample("Slack");
        assert_eq!(
            policy.resolve_blur_level(false, Some(&window), "active"),
            BlurLevel::Full
        );
    }

    #[test]
    fn resolve_blur_level_dev_app() {
        let policy = BlurPolicyConfig::load(None);
        let window = sample("Cursor");
        assert_eq!(
            policy.resolve_blur_level(false, Some(&window), "active"),
            BlurLevel::None
        );
    }
}
