//! Case-sensitive path verification used by the resolver.
//!
//! Prevents CI failures on case-sensitive Linux when development is done on
//! macOS/Windows (case-insensitive filesystems). The check is only active
//! when `case_sensitive_check = true` in the build configuration.
//!
//! On Linux (case-sensitive FS) the directory-scan overhead is skipped.
//!
//! Per SPEC-048: This module replaces `arkdown-plugin-sensitive-check`.
//! Case checking is folded into OHOS resolution via `arkdown-resolver::case_check`.

use std::path::Path;

/// The result of a case-sensitivity check for a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseCheckResult {
    /// The path matches the filesystem case exactly.
    Ok,
    /// On Linux where FS is always case-sensitive, the check is skipped.
    SkippedOnLinux,
    /// The filesystem entry exists but with a different case.
    Mismatch {
        /// The file name as written in the import specifier.
        specifier_name: String,
        /// The actual name on disk.
        actual_name: String,
    },
    /// The parent directory does not exist; cannot check.
    ParentNotFound,
}

/// Directory-entry based case check policy.
pub struct CaseCheckPolicy;

impl CaseCheckPolicy {
    /// Check whether the last path component of `path` matches the actual
    /// filesystem entry case.
    ///
    /// On Linux the check always returns [`CaseCheckResult::SkippedOnLinux`]
    /// because the OS enforces case sensitivity.
    ///
    /// On macOS and Windows (case-insensitive by default), the function reads the
    /// parent directory to find the actual-case entry.
    pub fn check_path_case(path: &Path) -> CaseCheckResult {
        #[cfg(target_os = "linux")]
        {
            let _ = path;
            return CaseCheckResult::SkippedOnLinux;
        }

        #[cfg(not(target_os = "linux"))]
        check_case_impl(path)
    }
}

#[cfg(not(target_os = "linux"))]
fn check_case_impl(path: &Path) -> CaseCheckResult {
    let parent = match path.parent().filter(|p| !p.as_os_str().is_empty()) {
        Some(p) => p,
        None => return CaseCheckResult::ParentNotFound,
    };
    let file_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return CaseCheckResult::ParentNotFound,
    };

    let entries = match std::fs::read_dir(parent) {
        Ok(e) => e,
        Err(_) => return CaseCheckResult::ParentNotFound,
    };

    for entry in entries.flatten() {
        let actual_os = entry.file_name();
        let Some(actual_name) = actual_os.to_str() else {
            continue;
        };
        if actual_name.to_lowercase() == file_name.to_lowercase() && actual_name != file_name {
            return CaseCheckResult::Mismatch {
                specifier_name: file_name.to_string(),
                actual_name: actual_name.to_string(),
            };
        }
    }

    CaseCheckResult::Ok
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nonexistent_parent_does_not_error() {
        let result =
            CaseCheckPolicy::check_path_case(Path::new("/nonexistent_dir_arkdown_test/Module.ts"));
        // Must not panic; result should indicate parent not found or skipped.
        assert!(matches!(
            result,
            CaseCheckResult::ParentNotFound | CaseCheckResult::SkippedOnLinux
        ));
    }

    #[test]
    fn test_correct_case_passes() {
        let tmp = std::env::temp_dir().join("arkdown_case_check_test");
        let _ = std::fs::create_dir_all(&tmp);
        let file = tmp.join("MyModule.ts");
        let _ = std::fs::write(&file, "");
        let result = CaseCheckPolicy::check_path_case(&file);
        assert!(
            matches!(
                result,
                CaseCheckResult::Ok | CaseCheckResult::SkippedOnLinux
            ),
            "expected Ok or SkippedOnLinux, got: {:?}",
            result
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
