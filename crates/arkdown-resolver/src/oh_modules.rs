use std::path::{Path, PathBuf};

/// Read the `main` field from an `oh-package.json5` file.
///
/// Returns `None` if the file cannot be read, parsed, or has no `main` field.
pub fn read_main_field(oh_package_path: &Path) -> Option<String> {
    let content = fs_err::read_to_string(oh_package_path).ok()?;
    let v: serde_json::Value = json5::from_str(&content).ok()?;
    v.get("main")?.as_str().map(|s| s.to_string())
}

/// Read the `main` field from an `oh-package.json5` or `package.json` file.
pub fn read_package_main_field(package_root: &Path) -> Option<String> {
    read_main_field(&package_root.join("oh-package.json5")).or_else(|| {
        let content = fs_err::read_to_string(package_root.join("package.json")).ok()?;
        let v: serde_json::Value = serde_json::from_str(&content).ok()?;
        v.get("main")?.as_str().map(|s| s.to_string())
    })
}

/// Read the package `name` field from an `oh-package.json5` file.
pub fn read_name_field(oh_package_path: &Path) -> Option<String> {
    let content = fs_err::read_to_string(oh_package_path).ok()?;
    let v: serde_json::Value = json5::from_str(&content).ok()?;
    v.get("name")?.as_str().map(|s| s.to_string())
}

/// Read file: dependencies from an `oh-package.json5` file.
pub fn read_file_dependencies(oh_package_path: &Path) -> Vec<(String, PathBuf)> {
    let Some(package_root) = oh_package_path.parent() else {
        return Vec::new();
    };
    let Ok(content) = fs_err::read_to_string(oh_package_path) else {
        return Vec::new();
    };
    let Ok(v) = json5::from_str::<serde_json::Value>(&content) else {
        return Vec::new();
    };

    let mut deps = Vec::new();
    for field in ["dependencies", "dynamicDependencies"] {
        let Some(map) = v.get(field).and_then(|deps| deps.as_object()) else {
            continue;
        };
        for (name, value) in map {
            let Some(raw) = value.as_str() else {
                continue;
            };
            let Some(relative_path) = raw.strip_prefix("file:") else {
                continue;
            };
            deps.push((name.clone(), package_root.join(relative_path)));
        }
    }
    deps
}

/// Read the `types` field from an `oh-package.json5` file.
pub fn read_types_field(oh_package_path: &Path) -> Option<String> {
    let content = fs_err::read_to_string(oh_package_path).ok()?;
    let v: serde_json::Value = json5::from_str(&content).ok()?;
    v.get("types")?.as_str().map(|s| s.to_string())
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

/// Read the package version from `oh-package.json5` or `package.json`.
pub fn read_package_version_field(package_root: &Path) -> String {
    let oh_package_path = package_root.join("oh-package.json5");
    let version = read_version_field(&oh_package_path);
    if !version.is_empty() {
        return version;
    }

    let content = match fs_err::read_to_string(package_root.join("package.json")) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    let v: serde_json::Value = match serde_json::from_str(&content) {
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
/// hvigor marks ByteCode HAR packages with `metadata.byteCodeHar` and writes
/// the bytecode to `ets/modules.abc`. `metadata.abcPath`, `main: "*.abc"`, and
/// `index.abc` are accepted as compatibility fallbacks.
pub fn is_bytecode_har(oh_package_path: &Path) -> bool {
    read_bytecode_abc_path(oh_package_path).is_some()
}

/// Resolve the bytecode artifact path for a ByteCode HAR.
pub fn read_bytecode_abc_path(oh_package_path: &Path) -> Option<PathBuf> {
    let content = fs_err::read_to_string(oh_package_path).ok()?;
    let v: serde_json::Value = json5::from_str(&content).ok()?;
    let pkg_dir = oh_package_path.parent()?;

    if v.pointer("/metadata/byteCodeHar")
        .and_then(|d| d.as_bool())
        .unwrap_or(false)
    {
        let abc_path = pkg_dir.join("ets").join("modules.abc");
        if abc_path.exists() {
            return Some(abc_path);
        }
    }

    if let Some(abc_path) = v.pointer("/metadata/abcPath").and_then(|d| d.as_str()) {
        let abc_path = pkg_dir.join(abc_path);
        if abc_path.exists() {
            return Some(abc_path);
        }
    }

    if let Some(main) = v
        .get("main")
        .and_then(|m| m.as_str())
        .filter(|main| main.ends_with(".abc"))
    {
        let abc_path = pkg_dir.join(main);
        if abc_path.exists() {
            return Some(abc_path);
        }
    }

    let abc_path = pkg_dir.join("index.abc");
    abc_path.exists().then_some(abc_path)
}

/// Resolve the declaration file path for a ByteCode HAR.
///
/// `metadata.declarationEntry` is not a file path in hvigor; it is an array of
/// normalized OHM URLs used to keep runtime-only records alive. For editor and
/// type-resolution metadata, prefer `types`, then `index.d.ets`.
pub fn read_declaration_entry(oh_package_path: &Path) -> Option<PathBuf> {
    let pkg_dir = oh_package_path.parent()?;
    if let Some(types) = read_types_field(oh_package_path) {
        return Some(pkg_dir.join(types));
    }
    Some(pkg_dir.join("index.d.ets"))
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
    fn test_is_bytecode_har_hvigor_marker() {
        let (dir, path) = write_temp(
            r#"{ "name": "foo", "version": "1.0.0", "metadata": { "byteCodeHar": true } }"#,
        );
        std::fs::create_dir_all(dir.path().join("ets")).unwrap();
        std::fs::write(
            dir.path().join("ets").join("modules.abc"),
            b"PANDA\x00\x00\x00",
        )
        .unwrap();
        assert!(is_bytecode_har(&path));
        assert!(read_bytecode_abc_path(&path)
            .expect("abc path")
            .ends_with("ets/modules.abc"));
    }

    #[test]
    fn test_is_bytecode_har_main_abc() {
        let json = r#"{ "name": "foo", "version": "1.0.0", "main": "dist/index.abc" }"#;
        let (dir, path) = write_temp(json);
        std::fs::create_dir_all(dir.path().join("dist")).unwrap();
        std::fs::write(
            dir.path().join("dist").join("index.abc"),
            b"PANDA\x00\x00\x00",
        )
        .unwrap();
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

    #[test]
    fn test_read_declaration_entry_uses_types_not_metadata_declaration_entry() {
        let json = r#"{
          "name": "foo",
          "types": "dist/index.d.ts",
          "metadata": {
            "declarationEntry": [
              "@normalized:N&entry&com.example&foo/src/main/ets/Worker&1.0.0"
            ]
          }
        }"#;
        let (dir, path) = write_temp(json);
        assert_eq!(
            read_declaration_entry(&path),
            Some(dir.path().join("dist/index.d.ts"))
        );
    }
}
