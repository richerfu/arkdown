use super::compile_arkts_package_entries::PackageEntryInfos;
use super::*;

pub(super) struct CompileArkTsInfo {
    pub(super) ts_path: PathBuf,
    pub(super) record_name: String,
    pub(super) module_type: String,
    pub(super) source_file: String,
    pub(super) package_name: String,
    pub(super) origin_source_lang: String,
    pub(super) abc_cache_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct CompileArkTsHandoff {
    pub(super) inputs: Vec<Ts2abcInput>,
    pub(super) npm_entries_path: Option<PathBuf>,
}

pub(super) fn write_compile_arkts_abc_handoff(
    cache_root: &Path,
    infos: &[CompileArkTsInfo],
    pkg_entries: &PackageEntryInfos,
) -> Result<CompileArkTsHandoff> {
    fs_err::create_dir_all(cache_root)?;
    let cache_root = canonical_or_owned(cache_root);

    let mut files_info = String::new();
    let mut modules_cache = String::new();
    let mut inputs = Vec::with_capacity(infos.len());

    for info in infos {
        let ts_path = canonical_or_owned(&info.ts_path);
        let abc_cache_path = info
            .abc_cache_path
            .clone()
            .unwrap_or_else(|| ts_path.with_extension("protoBin"));
        files_info.push_str(&format!(
            "{};{};{};{};{};false;{}\n",
            path_string(&ts_path),
            info.record_name,
            info.module_type,
            info.source_file,
            info.package_name,
            info.origin_source_lang
        ));
        modules_cache.push_str(&format!(
            "{};{}\n",
            path_string(&ts_path),
            path_string(&abc_cache_path)
        ));
        inputs.push(Ts2abcInput {
            path: ts_path,
            record_name: info.record_name.clone(),
            source_file: Some(info.source_file.clone()),
            source_map: None,
        });
    }

    fs_err::write(cache_root.join("filesInfo.txt"), files_info)?;

    let npm_entries_path = cache_root.join("npmEntries.txt");
    let mut npm_entries = String::new();
    for (entry_path, build_path) in pkg_entries.iter() {
        npm_entries.push_str(entry_path);
        npm_entries.push(':');
        npm_entries.push_str(build_path);
        npm_entries.push('\n');
    }
    fs_err::write(&npm_entries_path, npm_entries)?;
    modules_cache.push_str(&format!(
        "{};{}\n",
        path_string(&npm_entries_path),
        path_string(&npm_entries_path.with_extension("protoBin"))
    ));
    fs_err::write(cache_root.join("modules.cache"), modules_cache)?;

    Ok(CompileArkTsHandoff {
        inputs,
        npm_entries_path: Some(npm_entries_path),
    })
}

pub(super) fn read_compile_arkts_source_maps(
    module: &ModuleConfig,
    config: &Arc<ArkdownConfig>,
) -> Result<Option<String>> {
    let source_maps_json = compile_arkts_source_maps_json_path(module, config);
    if !source_maps_json.exists() {
        return Ok(None);
    }

    let content = fs_err::read_to_string(&source_maps_json)?;
    let mut output = serde_json::Map::new();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let entry: serde_json::Value = serde_json::from_str(line)
            .with_context(|| format!("failed to parse {}", source_maps_json.display()))?;
        let Some(key) = entry.get("key").and_then(|value| value.as_str()) else {
            continue;
        };
        let Some(val) = entry.get("val") else {
            continue;
        };
        output.insert(key.to_string(), val.clone());
    }

    if output.is_empty() {
        return Ok(None);
    }

    Ok(Some(serde_json::to_string_pretty(&output)?))
}

fn compile_arkts_source_maps_json_path(
    module: &ModuleConfig,
    config: &Arc<ArkdownConfig>,
) -> PathBuf {
    let mode_dir = match config.build_mode {
        BuildMode::Debug => "debug",
        BuildMode::Release => "release",
    };
    module
        .src_path
        .join("build/default/cache/default/default@CompileArkTS/esmodule")
        .join(mode_dir)
        .join("sourceMaps.json")
}
