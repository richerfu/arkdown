use rustc_hash::FxHashMap;
use std::{
    fmt,
    path::{Path, PathBuf},
};

use arkdown_path::{ArtifactPathSerializer, OhmUrl};

const DEFAULT_VERSION: &str = "1.0.0";

#[derive(Debug, Clone)]
pub struct HvigorSourceMapScope {
    pub module_name: String,
    pub package_name: String,
    pub version: String,
}

impl HvigorSourceMapScope {
    pub fn new(module_name: String, package_name: String, version: String) -> Self {
        Self {
            module_name,
            package_name,
            version,
        }
    }

    pub fn from_record_name(record_name: &str) -> Option<Self> {
        let parts = Self::record_name_parts(record_name)?;
        let module_name = parts.module_name.to_string();
        Some(Self::new(
            module_name.clone(),
            module_name,
            DEFAULT_VERSION.to_string(),
        ))
    }

    pub fn source_file_from_record_name(record_name: &str) -> Option<String> {
        let parts = Self::record_name_parts(record_name)?;
        let scope = Self::from_record_name(record_name)?;
        Some(scope.source_file_with_src_main_path(&Self::src_main_path(parts.ets_rel)))
    }

    pub fn normalize_debug_source_file(record_name: &str, source_file: &str) -> String {
        if HvigorSourceMapSourceKey::is_key(source_file) {
            return source_file.to_string();
        }

        let source_file = Self::source_file_path(source_file);
        if !(source_file.starts_with(Self::ets_dir())
            || source_file.starts_with(Self::src_main_ets_dir()))
        {
            return Self::path_to_slash(&source_file);
        }

        let scope = Self::from_record_name(record_name).unwrap_or_else(|| {
            Self::new(
                "entry".to_string(),
                "entry".to_string(),
                DEFAULT_VERSION.to_string(),
            )
        });
        let src_path = if source_file.starts_with(Self::src_main_dir()) {
            Self::with_ts_extension_path(&source_file)
        } else {
            Self::src_main_dir().join(Self::with_ts_extension_path(&source_file))
        };
        scope.source_file_with_src_main_path(&Self::path_to_slash(&src_path))
    }

    pub fn key_for_source_file(&self, source_file: &str) -> String {
        let source_file = Self::source_file_path(source_file);
        let src_path = Self::src_main_dir().join(Self::with_ts_extension_path(&source_file));
        self.source_file_with_src_main_path(&Self::path_to_slash(&src_path))
    }

    pub fn source_for_source_file(&self, source_file: &str) -> String {
        let source_path = Self::src_main_dir().join(Self::source_file_path(source_file));
        let mut path = PathBuf::from(&self.module_name);
        path.push(source_path);
        Self::path_to_slash(&path)
    }

    pub fn entry_package_info(&self) -> String {
        HvigorSourceMapPackageKey::new(&self.module_name, &self.version).to_key()
    }

    fn source_file_with_src_main_path(&self, src_main_path: &str) -> String {
        HvigorSourceMapSourceKey::new(
            &self.module_name,
            &self.package_name,
            &self.version,
            src_main_path,
        )
        .to_key()
    }

    fn record_name_parts(record_name: &str) -> Option<RecordNameParts<'_>> {
        if let Some(parts) = OhmUrl::parse_bundle_parts(record_name) {
            return Some(RecordNameParts {
                module_name: parts.module_name,
                ets_rel: parts.path,
            });
        }

        LegacyRecordName::new(record_name).parts()
    }

    fn src_main_path(ets_rel: &str) -> String {
        let ets_rel = Self::source_file_path(ets_rel);
        let src_path = if ets_rel.starts_with(Self::ets_dir()) {
            Self::src_main_dir().join(Self::with_ts_extension_path(&ets_rel))
        } else {
            Self::src_main_ets_dir().join(Self::with_ts_extension_path(&ets_rel))
        };
        Self::path_to_slash(&src_path)
    }

    fn with_ts_extension_path(path: &Path) -> PathBuf {
        let mut path = path.to_path_buf();
        path.set_extension("ts");
        path
    }

    fn source_file_path(source_file: &str) -> PathBuf {
        let mut path = PathBuf::new();
        for component in Path::new(source_file).components() {
            if matches!(component, std::path::Component::CurDir) {
                continue;
            }
            path.push(component.as_os_str());
        }
        path
    }

    fn path_to_slash(path: &Path) -> String {
        ArtifactPathSerializer::rel_to_slash(path)
    }

    fn ets_dir() -> PathBuf {
        PathBuf::from("ets")
    }

    fn src_main_dir() -> PathBuf {
        PathBuf::from("src").join("main")
    }

    fn src_main_ets_dir() -> PathBuf {
        Self::src_main_dir().join("ets")
    }
}

struct HvigorSourceMapPackageKey<'a> {
    module_name: &'a str,
    version: &'a str,
}

impl<'a> HvigorSourceMapPackageKey<'a> {
    fn new(module_name: &'a str, version: &'a str) -> Self {
        Self {
            module_name,
            version,
        }
    }

    fn to_key(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for HvigorSourceMapPackageKey<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}|{}", self.module_name, self.version)
    }
}

struct HvigorSourceMapSourceKey<'a> {
    module_name: &'a str,
    package_name: &'a str,
    version: &'a str,
    src_main_path: &'a str,
}

impl<'a> HvigorSourceMapSourceKey<'a> {
    fn new(
        module_name: &'a str,
        package_name: &'a str,
        version: &'a str,
        src_main_path: &'a str,
    ) -> Self {
        Self {
            module_name,
            package_name,
            version,
            src_main_path,
        }
    }

    fn to_key(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for HvigorSourceMapSourceKey<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}|{}|{}|{}",
            self.module_name, self.package_name, self.version, self.src_main_path
        )
    }
}

impl HvigorSourceMapSourceKey<'_> {
    const FIELD_SEPARATOR: char = '|';

    fn is_key(value: &str) -> bool {
        value.chars().any(|ch| ch == Self::FIELD_SEPARATOR)
    }
}

struct LegacyRecordName<'a> {
    raw: &'a str,
}

impl<'a> LegacyRecordName<'a> {
    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn parts(&self) -> Option<RecordNameParts<'a>> {
        let bundle_end = self.raw.find('/')?;
        let after_bundle = self.raw.get(bundle_end + 1..)?;
        let module_end = after_bundle.find('/')?;
        Some(RecordNameParts {
            module_name: after_bundle.get(..module_end)?,
            ets_rel: after_bundle.get(module_end + 1..)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct HvigorSourceMapOptions {
    pub module_name: String,
    pub package_name: String,
    pub version: String,
    pub chunk_source_files: FxHashMap<String, String>,
}

impl HvigorSourceMapOptions {
    /// Serialize source map entries to the hvigor per-file JSON format.
    pub fn to_json(
        &self,
        source_maps: &[HvigorSourceMapEntry],
    ) -> Result<String, serde_json::Error> {
        let opts = self;
        let mut map = serde_json::Map::new();

        for source_map in source_maps {
            let (key, value) = opts.entry_json(source_map);
            map.insert(key, value);
        }

        serde_json::to_string_pretty(&map)
    }

    pub fn entry_json(&self, source_map: &HvigorSourceMapEntry) -> (String, serde_json::Value) {
        let default_scope = HvigorSourceMapScope::new(
            self.module_name.clone(),
            self.package_name.clone(),
            self.version.clone(),
        );
        let source_file = source_map
            .source_file
            .as_deref()
            .map(HvigorSourceMapScope::source_file_path)
            .map(|source_file| HvigorSourceMapScope::path_to_slash(&source_file))
            .or_else(|| {
                self.chunk_source_files
                    .get(&source_map.chunk_name)
                    .map(|source_file| HvigorSourceMapScope::source_file_path(source_file))
                    .map(|source_file| HvigorSourceMapScope::path_to_slash(&source_file))
            })
            .or_else(|| {
                source_map
                    .source_map_source_file
                    .as_deref()
                    .map(|source_file| source_file.to_string())
            })
            .unwrap_or_else(|| source_map.chunk_name.clone());

        let scope = source_map
            .package_name
            .as_ref()
            .zip(source_map.package_version.as_ref())
            .map(|(package_name, version)| {
                HvigorSourceMapScope::new(
                    self.module_name.clone(),
                    package_name.clone(),
                    version.clone(),
                )
            })
            .unwrap_or_else(|| default_scope.clone());
        let key = scope.key_for_source_file(&source_file);
        let file = Path::new(&source_file)
            .file_name()
            .and_then(|file| file.to_str())
            .unwrap_or(&source_file);
        let sources = source_map
            .source_map_source_file
            .as_ref()
            .map(|source_file| vec![source_file.clone()])
            .unwrap_or_else(|| vec![scope.source_for_source_file(&source_file)]);
        let mut value = serde_json::json!({
            "version": 3,
            "file": file,
            "sourceRoot": "",
            "sources": sources,
            "names": [],
            "mappings": source_map.mappings,
            "entry-package-info": default_scope.entry_package_info(),
        });
        if let Some(package_info) = source_map.package_info() {
            value["package-info"] = serde_json::Value::String(package_info);
        }
        (key, value)
    }
}

#[derive(Debug, Clone)]
pub struct HvigorSourceMapEntry {
    pub chunk_name: String,
    pub mappings: String,
    pub source_file: Option<String>,
    pub source_map_source_file: Option<String>,
    pub package_name: Option<String>,
    pub package_version: Option<String>,
}

impl HvigorSourceMapEntry {
    fn package_info(&self) -> Option<String> {
        self.package_name
            .as_ref()
            .zip(self.package_version.as_ref())
            .map(|(name, version)| format!("{name}|{version}"))
    }
}

/// Writer that serializes hvigor-format source maps to JSON.
///
/// Owns hvigor source map serialization for SPEC-048 crate boundaries.
///
/// # Upstream correspondence
///
/// Corresponds to `arkts-pack.js::getOutputConfig` / `sourceMaps.map` JSON
/// serialization logic in hvigor release builds.
#[derive(Debug, Clone)]
pub struct HvigorSourceMapWriter {
    options: HvigorSourceMapOptions,
}

impl HvigorSourceMapWriter {
    /// Create a new writer from the given source map options.
    pub fn new(options: HvigorSourceMapOptions) -> Self {
        Self { options }
    }

    /// Serialize source map entries to the hvigor per-file JSON format.
    ///
    /// Each entry key is the hvigor-format
    /// `"module|package|version|src/main/ets/..."` path.
    pub fn to_json(
        &self,
        source_maps: &[HvigorSourceMapEntry],
    ) -> Result<String, serde_json::Error> {
        self.options.to_json(source_maps)
    }
}

struct RecordNameParts<'a> {
    module_name: &'a str,
    ets_rel: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_hvigor_debug_source_file_from_ohm_record() {
        assert_eq!(
            HvigorSourceMapScope::source_file_from_record_name(
                "@bundle:com.example.app/entry/ets/pages/Index"
            )
            .as_deref(),
            Some("entry|entry|1.0.0|src/main/ets/pages/Index.ts")
        );
    }

    #[test]
    fn normalizes_relative_debug_source_file_for_record_scope() {
        assert_eq!(
            HvigorSourceMapScope::normalize_debug_source_file(
                "@bundle:com.example.app/feature/ets/entryability/EntryAbility",
                "./ets/entryability/EntryAbility.ets"
            ),
            "feature|feature|1.0.0|src/main/ets/entryability/EntryAbility.ts"
        );
    }

    #[test]
    fn preserves_non_ets_and_already_scoped_debug_source_files() {
        assert_eq!(
            HvigorSourceMapScope::normalize_debug_source_file(
                "@bundle:com.example.app/entry/ets/pages/Index",
                "entry|entry|1.0.0|src/main/ets/pages/Index.ts"
            ),
            "entry|entry|1.0.0|src/main/ets/pages/Index.ts"
        );
        assert_eq!(
            HvigorSourceMapScope::normalize_debug_source_file(
                "@bundle:com.example.app/entry/ets/pages/Index",
                "generated/Index.js"
            ),
            "generated/Index.js"
        );
    }
}
