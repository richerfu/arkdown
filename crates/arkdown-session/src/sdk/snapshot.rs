//! [`SdkSnapshot`] — selected SDK, tool paths, and API version metadata.

use std::path::PathBuf;
use thiserror::Error;

/// Errors when building the SDK snapshot.
#[derive(Debug, Error)]
pub enum SdkSnapshotError {
    #[error("OHOS SDK not found: set $OHOS_SDK_HOME")]
    SdkNotFound,
}

/// SDK metadata loaded once at session startup.
#[derive(Debug, Clone)]
pub struct SdkSnapshot {
    /// Root SDK directory.
    pub sdk_root: Option<PathBuf>,
    /// API version (numeric).
    pub api_version: u32,
    /// Compile SDK version string (e.g., `"5.0.0.47"`).
    pub compile_sdk_version: String,
    /// Whether this is a placeholder with no real SDK.
    pub is_placeholder: bool,
}

impl SdkSnapshot {
    /// Build from environment variables (`$OHOS_SDK_HOME`).
    pub fn from_env() -> Result<Self, SdkSnapshotError> {
        let sdk_root = std::env::var_os("OHOS_SDK_HOME").map(PathBuf::from);
        if !sdk_root.as_ref().is_some_and(|p| p.is_dir()) {
            return Err(SdkSnapshotError::SdkNotFound);
        }
        Ok(Self {
            sdk_root,
            api_version: 12,
            compile_sdk_version: "5.0.0.47".to_string(),
            is_placeholder: false,
        })
    }

    /// Construct a placeholder snapshot when no SDK is configured.
    ///
    /// Used in tests and environments where the SDK is not required.
    pub fn placeholder() -> Self {
        Self {
            sdk_root: None,
            api_version: 12,
            compile_sdk_version: "5.0.0.47".to_string(),
            is_placeholder: true,
        }
    }
}
