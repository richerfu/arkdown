use std::collections::HashMap;
use std::path::Path;

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
        let parts = record_name_parts(record_name)?;
        let module_name = parts.module_name.to_string();
        Some(Self::new(
            module_name.clone(),
            module_name,
            DEFAULT_VERSION.to_string(),
        ))
    }

    pub fn source_file_from_record_name(record_name: &str) -> Option<String> {
        let parts = record_name_parts(record_name)?;
        let scope = Self::from_record_name(record_name)?;
        Some(scope.source_file_with_src_main_path(&src_main_path(parts.ets_rel)))
    }

    pub fn normalize_debug_source_file(record_name: &str, source_file: &str) -> String {
        if source_file.contains('|') {
            return source_file.to_string();
        }

        let source_file = source_file.trim_start_matches("./");
        if !(source_file.starts_with("ets/") || source_file.starts_with("src/main/ets/")) {
            return source_file.to_string();
        }

        let scope = Self::from_record_name(record_name).unwrap_or_else(|| {
            Self::new(
                "entry".to_string(),
                "entry".to_string(),
                DEFAULT_VERSION.to_string(),
            )
        });
        let src_path = if source_file.starts_with("src/main/") {
            with_ts_extension(source_file)
        } else {
            format!("src/main/{}", with_ts_extension(source_file))
        };
        scope.source_file_with_src_main_path(&src_path)
    }

    pub fn key_for_source_file(&self, source_file: &str) -> String {
        self.source_file_with_src_main_path(&format!("src/main/{}", with_ts_extension(source_file)))
    }

    pub fn source_for_source_file(&self, source_file: &str) -> String {
        format!("{}/src/main/{}", self.module_name, source_file)
    }

    pub fn entry_package_info(&self) -> String {
        format!("{}|{}", self.module_name, self.version)
    }

    fn source_file_with_src_main_path(&self, src_main_path: &str) -> String {
        format!(
            "{}|{}|{}|{}",
            self.module_name, self.package_name, self.version, src_main_path
        )
    }
}

#[derive(Debug, Clone)]
pub struct HvigorSourceMapOptions {
    pub module_name: String,
    pub package_name: String,
    pub version: String,
    pub chunk_source_files: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct HvigorSourceMapEntry {
    pub chunk_name: String,
    pub mappings: String,
    pub source_file: Option<String>,
    pub source_map_source_file: Option<String>,
}

pub fn hvigor_source_maps_json(
    source_maps: &[HvigorSourceMapEntry],
    opts: &HvigorSourceMapOptions,
) -> Result<String, serde_json::Error> {
    let mut map = serde_json::Map::new();
    let scope = HvigorSourceMapScope::new(
        opts.module_name.clone(),
        opts.package_name.clone(),
        opts.version.clone(),
    );

    for source_map in source_maps {
        let source_file = source_map
            .source_file
            .as_deref()
            .map(|source_file| source_file.trim_start_matches("./").to_string())
            .or_else(|| {
                opts.chunk_source_files
                    .get(&source_map.chunk_name)
                    .map(|source_file| source_file.trim_start_matches("./").to_string())
            })
            .or_else(|| {
                source_map
                    .source_map_source_file
                    .as_deref()
                    .map(|source_file| source_file.to_string())
            })
            .unwrap_or_else(|| source_map.chunk_name.clone());

        let key = scope.key_for_source_file(&source_file);
        let file = Path::new(&source_file)
            .file_name()
            .and_then(|file| file.to_str())
            .unwrap_or(&source_file);

        map.insert(
            key,
            serde_json::json!({
                "version": 3,
                "file": file,
                "sourceRoot": "",
                "sources": [scope.source_for_source_file(&source_file)],
                "names": [],
                "mappings": source_map.mappings,
                "entry-package-info": scope.entry_package_info(),
            }),
        );
    }

    serde_json::to_string_pretty(&map)
}

struct RecordNameParts<'a> {
    module_name: &'a str,
    ets_rel: &'a str,
}

fn record_name_parts(record_name: &str) -> Option<RecordNameParts<'_>> {
    let record_name = record_name.strip_prefix("@bundle:").unwrap_or(record_name);
    let mut parts = record_name.splitn(3, '/');
    let _bundle_name = parts.next()?;
    let module_name = parts.next()?;
    let ets_rel = parts.next()?;
    Some(RecordNameParts {
        module_name,
        ets_rel,
    })
}

fn src_main_path(ets_rel: &str) -> String {
    if ets_rel.starts_with("ets/") {
        format!("src/main/{}.ts", ets_rel)
    } else {
        format!("src/main/ets/{}.ts", ets_rel)
    }
}

fn with_ts_extension(path: &str) -> String {
    path.rsplit_once('.')
        .map(|(stem, _)| format!("{stem}.ts"))
        .unwrap_or_else(|| format!("{path}.ts"))
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
