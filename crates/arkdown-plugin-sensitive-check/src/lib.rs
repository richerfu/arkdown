use std::borrow::Cow;
use std::path::Path;

use rolldown_plugin::{
    HookLoadArgs, HookLoadReturn, HookUsage, Plugin, SharedLoadPluginContext,
};

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

impl std::fmt::Debug for SensitiveCheckPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SensitiveCheckPlugin")
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl SensitiveCheckPlugin {
    pub fn new(enabled: bool) -> Self {
        SensitiveCheckPlugin { enabled }
    }

    fn check_case(id: &str) -> Option<CaseMismatchError> {
        let path = Path::new(id);
        let parent = path.parent().filter(|p| !p.as_os_str().is_empty())?;
        let file_name = path.file_name()?.to_str()?;

        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                let actual_os = entry.file_name();
                let actual = actual_os.to_string_lossy();
                if actual.to_lowercase() == file_name.to_lowercase() && actual != file_name {
                    return Some(CaseMismatchError {
                        specifier: id.to_string(),
                        expected: file_name.to_string(),
                        actual: actual.into_owned(),
                    });
                }
            }
        }
        None
    }
}

impl Default for SensitiveCheckPlugin {
    fn default() -> Self {
        Self::new(true)
    }
}

fn check_case_sensitive(id: &str) -> HookLoadReturn {
    // On Linux the FS is already case-sensitive — skip the check entirely.
    #[cfg(target_os = "linux")]
    let _ = id;
    #[cfg(target_os = "linux")]
    return Ok(None);

    #[cfg(not(target_os = "linux"))]
    if let Some(err) = SensitiveCheckPlugin::check_case(id) {
        return Err(anyhow::anyhow!("{err}"));
    }
    Ok(None)
}

impl Plugin for SensitiveCheckPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("sensitive-check")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Load
    }

    fn load(
        &self,
        _ctx: SharedLoadPluginContext,
        args: &HookLoadArgs<'_>,
    ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
        let enabled = self.enabled;
        let id = args.id.to_owned();
        async move {
            if !enabled {
                return Ok(None);
            }
            check_case_sensitive(&id)
        }
    }
}

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

    #[test]
    fn test_disabled_plugin_passthrough() {
        let plugin = SensitiveCheckPlugin::new(false);
        assert!(!plugin.enabled);
    }

    #[test]
    fn test_nonexistent_parent_does_not_error() {
        let result = SensitiveCheckPlugin::check_case(
            "/nonexistent_dir_arkdown_test/Module.ts",
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_correct_case_passes() {
        let tmp = std::env::temp_dir().join("arkdown_sensitive_test");
        let _ = std::fs::create_dir_all(&tmp);
        let file = tmp.join("MyModule.ts");
        let _ = std::fs::write(&file, "");
        let id = file.to_string_lossy().to_string();
        let result = SensitiveCheckPlugin::check_case(&id);
        assert!(result.is_none());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
