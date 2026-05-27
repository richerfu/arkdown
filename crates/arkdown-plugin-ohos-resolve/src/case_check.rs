use std::path::Path;

/// Verify that a resolved path's filename matches the actual filesystem case.
///
/// Returns the actual filename if it differs from the expected name (case mismatch),
/// or `None` if the case is correct or the check cannot be performed.
pub fn check_case(resolved: &Path) -> Option<String> {
    let parent = resolved.parent()?;
    let expected = resolved.file_name()?.to_string_lossy();
    std::fs::read_dir(parent).ok()?.find_map(|e| {
        let actual = e.ok()?.file_name();
        let actual_str = actual.to_string_lossy().to_string();
        if actual_str.to_lowercase() == expected.to_lowercase() && actual_str != expected.as_ref() {
            Some(actual_str)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_case_check_on_existing_path() {
        // Test with a known existing path — no mismatch expected.
        // Just ensure no panic.
        let path = std::env::current_dir().unwrap().join("Cargo.toml");
        let _ = check_case(&path);
    }
}
