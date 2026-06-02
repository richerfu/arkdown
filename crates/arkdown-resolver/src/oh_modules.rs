//! Parsing and querying of `oh-package.json5` package manifests.
//!
//! [`OhPackage`] is the single entry point: load the manifest once with
//! [`OhPackage::load`] or [`OhPackage::load_from_root`], then call its methods
//! to access package metadata.  This avoids reading and parsing the same file
//! repeatedly for every field lookup.

use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// oh-package.json5 deserialization
// ---------------------------------------------------------------------------

/// Metadata block inside `oh-package.json5`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OhPackageMetadata {
    /// `true` when the package is a pre-compiled ByteCode HAR.
    pub byte_code_har: bool,
    /// Explicit bytecode artifact path (relative to package root).
    pub abc_path: Option<String>,
}

/// Parsed contents of an `oh-package.json5` file.
///
/// Construct via [`OhPackage::load`] or [`OhPackage::load_from_root`].
/// All accessor methods operate on already-parsed data with no further I/O.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OhPackage {
    /// Package root directory — the directory that contains `oh-package.json5`.
    #[serde(skip)]
    pkg_dir: PathBuf,

    pub name: Option<String>,
    pub version: Option<String>,
    /// `main` entry point (source-based packages).
    pub main: Option<String>,
    /// TypeScript declaration entry (`types` / `typings` field).
    #[serde(alias = "typings")]
    pub types: Option<String>,
    /// ESM exports map.
    pub exports: Option<serde_json::Value>,
    pub dependencies: FxHashMap<String, String>,
    pub dynamic_dependencies: FxHashMap<String, String>,
    pub metadata: OhPackageMetadata,
}

impl Default for OhPackage {
    fn default() -> Self {
        OhPackage {
            pkg_dir: PathBuf::new(),
            name: None,
            version: None,
            main: None,
            types: None,
            exports: None,
            dependencies: FxHashMap::default(),
            dynamic_dependencies: FxHashMap::default(),
            metadata: OhPackageMetadata::default(),
        }
    }
}

impl OhPackage {
    /// Load and parse `oh-package.json5` at `path`.
    ///
    /// Returns `None` if the file cannot be read or parsed.
    pub fn load(path: &Path) -> Option<Self> {
        let content = fs_err::read_to_string(path).ok()?;
        let mut pkg: OhPackage = json5::from_str(&content).ok()?;
        pkg.pkg_dir = path.parent().unwrap_or(path).to_path_buf();
        Some(pkg)
    }

    /// Load `oh-package.json5` from `root/oh-package.json5`.
    ///
    /// Falls back to `root/package.json` if `oh-package.json5` is absent.
    pub fn load_from_root(root: &Path) -> Option<Self> {
        let oh_path = root.join("oh-package.json5");
        if oh_path.is_file() {
            return Self::load(&oh_path);
        }
        // Fallback: plain package.json (npm-style packages in oh_modules)
        let pkg_json = root.join("package.json");
        if pkg_json.is_file() {
            let content = fs_err::read_to_string(&pkg_json).ok()?;
            let mut pkg: OhPackage = serde_json::from_str(&content).ok()?;
            pkg.pkg_dir = root.to_path_buf();
            return Some(pkg);
        }
        None
    }

    // ── Metadata accessors ────────────────────────────────────────────────────

    /// Package name from the `name` field.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Package version, or an empty string when absent.
    pub fn version(&self) -> &str {
        self.version.as_deref().unwrap_or("")
    }

    // ── Entry point resolution ────────────────────────────────────────────────

    /// Main entry point from the `main` field.
    pub fn main_field(&self) -> Option<&str> {
        self.main.as_deref()
    }

    /// ESM entry from `exports["."].import` or `exports["."].default`.
    pub fn exports_import_field(&self) -> Option<String> {
        let exports = self.exports.as_ref()?;
        if let Some(root) = exports.get(".") {
            if let Some(s) = root.get("import").and_then(|v| v.as_str()) {
                return Some(s.to_string());
            }
            if let Some(s) = root.get("default").and_then(|v| v.as_str()) {
                return Some(s.to_string());
            }
        }
        exports.as_str().map(str::to_string)
    }

    // ── ByteCode HAR detection ────────────────────────────────────────────────

    /// Returns `true` when this package contains pre-compiled Panda bytecode.
    pub fn is_bytecode_har(&self) -> bool {
        self.bytecode_abc_path().is_some()
    }

    /// Resolve the `.abc` artifact path for a ByteCode HAR.
    ///
    /// Checks (in priority order):
    /// 1. `metadata.byteCodeHar == true` → `ets/modules.abc`
    /// 2. `metadata.abcPath` (explicit override)
    /// 3. `main` field ending with `.abc`
    /// 4. `index.abc` in package root
    ///
    /// Returns `None` if none of the above paths exist on disk.
    pub fn bytecode_abc_path(&self) -> Option<PathBuf> {
        if self.metadata.byte_code_har {
            let abc = self.pkg_dir.join("ets").join("modules.abc");
            if abc.exists() {
                return Some(abc);
            }
        }
        if let Some(ref rel) = self.metadata.abc_path {
            let abc = self.pkg_dir.join(rel);
            if abc.exists() {
                return Some(abc);
            }
        }
        if let Some(main) = self.main_field().filter(|m| m.ends_with(".abc")) {
            let abc = self.pkg_dir.join(main);
            if abc.exists() {
                return Some(abc);
            }
        }
        let abc = self.pkg_dir.join("index.abc");
        abc.exists().then_some(abc)
    }

    /// Resolve the TypeScript declaration entry for a ByteCode HAR.
    ///
    /// Prefers the `types` field; falls back to `index.d.ets`.
    pub fn declaration_entry(&self) -> Option<PathBuf> {
        if let Some(ref types) = self.types {
            return Some(self.pkg_dir.join(types));
        }
        Some(self.pkg_dir.join("index.d.ets"))
    }

    // ── Dependency enumeration ────────────────────────────────────────────────

    /// `file:` dependencies — returns `(alias_name, resolved_path)` pairs.
    ///
    /// Covers both `dependencies` and `dynamicDependencies`.
    pub fn file_dependencies(&self) -> Vec<(String, PathBuf)> {
        let mut deps = Vec::new();
        for map in [&self.dependencies, &self.dynamic_dependencies] {
            for (name, value) in map {
                if let Some(rel) = value.strip_prefix("file:") {
                    deps.push((name.clone(), self.pkg_dir.join(rel)));
                }
            }
        }
        deps
    }
}

// ---------------------------------------------------------------------------
// Directory search
// ---------------------------------------------------------------------------

/// Find the root directory of a named package within an `oh_modules/` tree.
///
/// Checks two locations:
/// - `root/<package_name>/oh-package.json5` (direct install)
/// - `root/.ohpm/oh_modules/<package_name>/oh-package.json5` (ohpm registry)
pub fn package_root_in_oh_modules(root: &Path, package_name: &str) -> Option<PathBuf> {
    let candidates = [
        root.join(package_name),
        root.join(".ohpm/oh_modules").join(package_name),
    ];
    candidates
        .into_iter()
        .find(|p| p.join("oh-package.json5").is_file() || p.join("package.json").is_file())
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
        let pkg = OhPackage::load(&path).unwrap();
        assert!(pkg.is_bytecode_har());
        assert!(pkg
            .bytecode_abc_path()
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
        let pkg = OhPackage::load(&path).unwrap();
        assert!(pkg.is_bytecode_har());
    }

    #[test]
    fn test_is_bytecode_har_normal() {
        let json = r#"{ "name": "foo", "version": "1.0.0", "main": "index.ets" }"#;
        let (_dir, path) = write_temp(json);
        let pkg = OhPackage::load(&path).unwrap();
        assert!(!pkg.is_bytecode_har());
    }

    #[test]
    fn test_main_field() {
        let json = r#"{ "name": "foo", "main": "src/index.ets" }"#;
        let (_dir, path) = write_temp(json);
        let pkg = OhPackage::load(&path).unwrap();
        assert_eq!(pkg.main_field(), Some("src/index.ets"));
    }

    #[test]
    fn test_declaration_entry_uses_types_not_metadata_declaration_entry() {
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
        let pkg = OhPackage::load(&path).unwrap();
        assert_eq!(
            pkg.declaration_entry(),
            Some(dir.path().join("dist/index.d.ts"))
        );
    }
}
