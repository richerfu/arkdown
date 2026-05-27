use std::path::{Path, PathBuf};

/// Read the `main` field from an `oh-package.json5` file.
///
/// Returns `None` if the file cannot be read, parsed, or has no `main` field.
pub fn read_main_field(oh_package_path: &Path) -> Option<String> {
    let content = fs_err::read_to_string(oh_package_path).ok()?;
    let v: serde_json::Value = json5::from_str(&content).ok()?;
    v.get("main")?.as_str().map(|s| s.to_string())
}

/// Read the `version` field from an `oh-package.json5` file.
///
/// Returns an empty string if the field is absent or unreadable.
pub fn read_version_field(oh_package_path: &Path) -> String {
    let content = match fs_err::read_to_string(oh_package_path) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    let v: serde_json::Value = match json5::from_str(&content) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    v.get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Returns `true` if this package is a ByteCode HAR.
///
/// Criteria: `oh-package.json5` has `metadata.declarationEntry` OR `main` ends in `.abc`.
pub fn is_bytecode_har(oh_package_path: &Path) -> bool {
    let content = match fs_err::read_to_string(oh_package_path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let v: serde_json::Value = match json5::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if v.pointer("/metadata/declarationEntry").is_some() {
        return true;
    }
    if let Some(main) = v.get("main").and_then(|m| m.as_str()) {
        if main.ends_with(".abc") {
            return true;
        }
    }
    false
}

/// Resolve the declaration entry path for a ByteCode HAR.
///
/// Returns `None` if the package is not a ByteCode HAR or the field is missing.
pub fn read_declaration_entry(oh_package_path: &Path) -> Option<PathBuf> {
    let content = fs_err::read_to_string(oh_package_path).ok()?;
    let v: serde_json::Value = json5::from_str(&content).ok()?;
    let decl = v
        .pointer("/metadata/declarationEntry")
        .and_then(|d| d.as_str())?;
    let pkg_dir = oh_package_path.parent()?;
    Some(pkg_dir.join(decl))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp(content: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oh-package.json5");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        (dir, path)
    }

    #[test]
    fn test_is_bytecode_har_declaration_entry() {
        let json = r#"{ "name": "foo", "version": "1.0.0", "main": "index.ets", "metadata": { "declarationEntry": "index.d.ets" } }"#;
        let (_dir, path) = write_temp(json);
        assert!(is_bytecode_har(&path));
    }

    #[test]
    fn test_is_bytecode_har_main_abc() {
        let json = r#"{ "name": "foo", "version": "1.0.0", "main": "dist/index.abc" }"#;
        let (_dir, path) = write_temp(json);
        assert!(is_bytecode_har(&path));
    }

    #[test]
    fn test_is_bytecode_har_normal() {
        let json = r#"{ "name": "foo", "version": "1.0.0", "main": "index.ets" }"#;
        let (_dir, path) = write_temp(json);
        assert!(!is_bytecode_har(&path));
    }

    #[test]
    fn test_read_main_field() {
        let json = r#"{ "name": "foo", "main": "src/index.ets" }"#;
        let (_dir, path) = write_temp(json);
        assert_eq!(read_main_field(&path), Some("src/index.ets".to_string()));
    }
}
