use std::path::Path;

use async_trait::async_trait;
use arkdown_core::{ArkdownPlugin, ArkdownError, HookLoadOutput, PluginContext, PluginResult};

/// Plugin that validates import specifiers match the actual filesystem case.
///
/// Prevents CI failures on case-sensitive Linux when development is done on
/// macOS/Windows (case-insensitive). Only active when `enabled = true`.
///
/// On Linux (case-sensitive FS) the check is skipped to avoid the overhead of
/// a `read_dir` call on every module load.
pub struct SensitiveCheckPlugin {
    pub enabled: bool,
}

impl SensitiveCheckPlugin {
    pub fn new(enabled: bool) -> Self {
        SensitiveCheckPlugin { enabled }
    }
}

impl Default for SensitiveCheckPlugin {
    fn default() -> Self {
        Self::new(true)
    }
}

#[async_trait]
impl ArkdownPlugin for SensitiveCheckPlugin {
    fn name(&self) -> &str {
        "sensitive-check"
    }

    async fn load(
        &self,
        _ctx: &PluginContext,
        id: &str,
    ) -> PluginResult<Option<HookLoadOutput>> {
        if !self.enabled {
            return Ok(None);
        }

        // On Linux, the filesystem is already case-sensitive — no check needed.
        #[cfg(target_os = "linux")]
        return Ok(None);

        let path = Path::new(id);
        let parent = match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => return Ok(None),
        };
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => return Ok(None),
        };

        // Read the actual directory entries and look for a case-insensitive match
        // that differs in case from the requested file name.
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                let actual_os = entry.file_name();
                let actual = actual_os.to_string_lossy();
                if actual.to_lowercase() == file_name.to_lowercase() && actual != file_name {
                    return Err(ArkdownError::PluginError {
                        plugin: "sensitive-check".to_string(),
                        source: Box::new(CaseMismatchError {
                            specifier: id.to_string(),
                            expected: file_name.to_string(),
                            actual: actual.into_owned(),
                        }),
                    });
                }
            }
        }

        Ok(None)
    }
}

/// Error emitted when an import specifier's case does not match the file on disk.
#[derive(Debug)]
struct CaseMismatchError {
    specifier: String,
    expected: String,
    actual: String,
}

impl std::fmt::Display for CaseMismatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Case mismatch for '{}': file on disk is '{}' but specifier uses '{}'",
            self.specifier, self.actual, self.expected
        )
    }
}

impl std::error::Error for CaseMismatchError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_disabled_plugin_passthrough() {
        let plugin = SensitiveCheckPlugin::new(false);
        let result = plugin
            .load(&PluginContext::default(), "/any/path/Module.ts")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_nonexistent_parent_does_not_error() {
        let plugin = SensitiveCheckPlugin::new(true);
        // Directory does not exist — read_dir silently returns Err, we pass through.
        let result = plugin
            .load(
                &PluginContext::default(),
                "/nonexistent_dir_arkdown_test/Module.ts",
            )
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_correct_case_passes() {
        let plugin = SensitiveCheckPlugin::new(true);
        let tmp = std::env::temp_dir().join("arkdown_sensitive_test");
        let _ = std::fs::create_dir_all(&tmp);
        let file = tmp.join("MyModule.ts");
        let _ = std::fs::write(&file, "");
        let id = file.to_string_lossy().to_string();
        // Exact case match — should not error.
        let result = plugin.load(&PluginContext::default(), &id).await.unwrap();
        assert!(result.is_none());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
