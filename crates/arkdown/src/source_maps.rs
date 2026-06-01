use std::collections::HashMap;
use std::path::Path;

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
    pub fallback_source_file: Option<String>,
}

pub fn hvigor_source_maps_json(
    source_maps: &[HvigorSourceMapEntry],
    opts: &HvigorSourceMapOptions,
) -> Result<String, serde_json::Error> {
    let mut map = serde_json::Map::new();

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
                    .fallback_source_file
                    .as_deref()
                    .map(|source_file| source_file.to_string())
            })
            .unwrap_or_else(|| source_map.chunk_name.clone());

        let key = format!(
            "{}|{}|{}|src/main/{}",
            opts.module_name,
            opts.package_name,
            opts.version,
            with_ts_extension(&source_file),
        );
        let file = Path::new(&source_file)
            .file_name()
            .and_then(|file| file.to_str())
            .unwrap_or(&source_file);
        let source = format!("{}/src/main/{}", opts.module_name, source_file);

        map.insert(
            key,
            serde_json::json!({
                "version": 3,
                "file": file,
                "sourceRoot": "",
                "sources": [source],
                "names": [],
                "mappings": source_map.mappings,
                "entry-package-info": format!("{}|{}", opts.module_name, opts.version),
            }),
        );
    }

    serde_json::to_string_pretty(&map)
}

fn with_ts_extension(path: &str) -> String {
    path.rsplit_once('.')
        .map(|(stem, _)| format!("{stem}.ts"))
        .unwrap_or_else(|| format!("{path}.ts"))
}
