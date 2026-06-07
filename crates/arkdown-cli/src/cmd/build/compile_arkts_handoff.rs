use super::compile_arkts_package_entries::PackageEntryInfos;
use super::*;
use std::fmt::Write;

pub(super) struct CompileArkTsInfo {
    pub(super) ts_path: PathBuf,
    pub(super) record_name: String,
    pub(super) module_type: String,
    pub(super) source_file: String,
    pub(super) package_name: String,
    pub(super) origin_source_lang: String,
    pub(super) is_shared_module: bool,
    pub(super) abc_cache_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct CompileArkTsHandoff {
    pub(super) inputs: Vec<Ts2abcInput>,
    pub(super) npm_entries_path: Option<PathBuf>,
    pub(super) modules_cache_path: Option<PathBuf>,
}

pub(super) fn write_compile_arkts_abc_handoff(
    cache_root: &Path,
    infos: &[CompileArkTsInfo],
    pkg_entries: &PackageEntryInfos,
) -> Result<CompileArkTsHandoff> {
    fs_err::create_dir_all(cache_root)?;
    let cache_root = canonical_or_owned(cache_root);

    let mut writer = CompileArkTsHandoffWriter::new(&cache_root);
    let mut inputs = Vec::with_capacity(infos.len());

    for info in infos {
        let ts_path = canonical_or_owned(&info.ts_path);
        let abc_cache_path = info
            .abc_cache_path
            .clone()
            .unwrap_or_else(|| ts_path.with_extension("protoBin"));
        writer.push_file_info(info, &ts_path, &abc_cache_path);
        inputs.push(Ts2abcInput {
            path: ts_path,
            output: None,
            cache_file: Some(abc_cache_path),
            record_name: info.record_name.clone(),
            source_file: Some(info.source_file.clone()),
            package_name: Some(info.package_name.clone()),
            is_commonjs: info.module_type == "commonjs",
            is_shared_module: info.is_shared_module,
            origin_source_lang: info.origin_source_lang.clone(),
            source_map: None,
        });
    }

    writer.write_files_info()?;
    let npm_entries_path = writer.write_npm_entries(pkg_entries)?;
    writer.push_modules_cache_entry(
        &npm_entries_path,
        &npm_entries_path.with_extension("protoBin"),
    );
    writer.write_modules_cache()?;

    Ok(CompileArkTsHandoff {
        inputs,
        npm_entries_path: Some(npm_entries_path),
        modules_cache_path: Some(cache_root.join("modules.cache")),
    })
}

struct CompileArkTsHandoffWriter<'a> {
    cache_root: &'a Path,
    files_info: String,
    modules_cache: String,
}

impl<'a> CompileArkTsHandoffWriter<'a> {
    fn new(cache_root: &'a Path) -> Self {
        Self {
            cache_root,
            files_info: String::new(),
            modules_cache: String::new(),
        }
    }

    fn push_file_info(&mut self, info: &CompileArkTsInfo, ts_path: &Path, abc_cache_path: &Path) {
        let _ = writeln!(
            self.files_info,
            "{};{};{};{};{};{};{}",
            path_string(ts_path),
            info.record_name,
            info.module_type,
            info.source_file,
            info.package_name,
            info.is_shared_module,
            info.origin_source_lang
        );
        self.push_modules_cache_entry(ts_path, abc_cache_path);
    }

    fn push_modules_cache_entry(&mut self, source: &Path, cache: &Path) {
        let _ = writeln!(
            self.modules_cache,
            "{};{}",
            path_string(source),
            path_string(cache)
        );
    }

    fn write_files_info(&self) -> Result<()> {
        fs_err::write(self.cache_root.join("filesInfo.txt"), &self.files_info)?;
        Ok(())
    }

    fn write_npm_entries(&self, pkg_entries: &PackageEntryInfos) -> Result<PathBuf> {
        let npm_entries_path = self.cache_root.join("npmEntries.txt");
        let mut npm_entries = String::new();
        for (entry_path, build_path) in pkg_entries.iter() {
            let _ = writeln!(npm_entries, "{entry_path}:{build_path}");
        }
        fs_err::write(&npm_entries_path, npm_entries)?;
        Ok(npm_entries_path)
    }

    fn write_modules_cache(&self) -> Result<()> {
        fs_err::write(self.cache_root.join("modules.cache"), &self.modules_cache)?;
        Ok(())
    }
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
