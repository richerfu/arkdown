use super::*;
use rustc_hash::FxHashMap;

pub(super) struct CompileArktsCacheWriter<'a> {
    loader_json: &'a arkdown_profile::LoaderJson,
    config: &'a Arc<ArkdownConfig>,
    cache_root: PathBuf,
    transformer: EtsTransformPlugin,
}

impl<'a> CompileArktsCacheWriter<'a> {
    pub(super) fn new(
        module: &ModuleConfig,
        loader_json: &'a arkdown_profile::LoaderJson,
        config: &'a Arc<ArkdownConfig>,
        resource_refs: &FxHashMap<String, ResourceReference>,
    ) -> Self {
        let cache_root = module
            .src_path
            .join("build/default/cache/default/default@CompileArkTS/esmodule")
            .join(Self::mode_dir(config.build_mode.clone()));
        let transformer =
            EtsTransformPlugin::with_resource_references(config.clone(), resource_refs.clone());

        Self {
            loader_json,
            config,
            cache_root,
            transformer,
        }
    }

    pub(super) fn write(&self, chunk_graph: &ChunkGraph) -> Result<CompileArkTsHandoff> {
        let module_ids = compile_module_ids(chunk_graph);
        if module_ids.is_empty() {
            return Err(anyhow::anyhow!(
                "CompileArkTS module graph is empty; hvigor compiles Rollup getModuleIds(), so arkdown cannot continue without rolldown module_ids"
            ));
        }

        let mut source_map_lines = Vec::new();
        let mut compile_infos = Vec::new();
        let mut pkg_entries = PackageEntryInfos::default();

        for module_id in module_ids {
            self.write_module(
                &module_id,
                &mut compile_infos,
                &mut pkg_entries,
                &mut source_map_lines,
            )?;
        }

        if compile_infos.is_empty() {
            return Err(anyhow::anyhow!(
                "CompileArkTS module graph had no compilable project modules"
            ));
        }

        self.write_source_maps(&source_map_lines)?;
        self.populate_package_entries(&mut pkg_entries);
        write_compile_arkts_abc_handoff(&self.cache_root, &compile_infos, &pkg_entries)
    }

    fn write_module(
        &self,
        module_id: &str,
        compile_infos: &mut Vec<CompileArkTsInfo>,
        pkg_entries: &mut PackageEntryInfos,
        source_map_lines: &mut Vec<String>,
    ) -> Result<()> {
        let Some(source_path) = module_id_to_source_path(module_id) else {
            return Ok(());
        };
        let Some(entry_info) = compile_module_info(
            &source_path,
            &self.cache_root,
            self.loader_json,
            self.config,
        ) else {
            return Ok(());
        };

        let source = fs_err::read_to_string(&entry_info.source_path).with_context(|| {
            format!(
                "failed to read CompileArkTS source {}",
                entry_info.source_path.display()
            )
        })?;
        let transformed = self.transform_source(module_id, &source, &entry_info)?;
        let code = self.output_code(&source, transformed.as_ref(), &entry_info);
        if matches!(entry_info.source_ext.as_str(), "ets" | "ts") && code.trim().is_empty() {
            return Ok(());
        }

        self.write_output_source(&entry_info, code)?;
        compile_infos.push(entry_info.to_compile_info());

        if !self.loader_json.use_normalized_ohm_url
            && is_package_entry_file(&entry_info.source_path)
        {
            add_graph_package_entry(pkg_entries, &entry_info, self.loader_json);
        }

        if let Some(source_map) = transformed.and_then(|output| output.map.into_sourcemap()) {
            source_map_lines.push(self.source_map_line(
                module_id,
                &entry_info,
                &source_map.to_json_string(),
            )?);
        }
        Ok(())
    }

    fn transform_source(
        &self,
        module_id: &str,
        source: &str,
        entry_info: &CompileModuleInfo,
    ) -> Result<Option<rolldown_plugin::HookTransformOutput>> {
        if !matches!(
            entry_info.source_ext.as_str(),
            "ets" | "ts" | "js" | "mjs" | "cjs"
        ) {
            return Ok(None);
        }

        self.transformer
            .do_compile_transform(source, module_id, entry_info.output_ext == "js")
            .with_context(|| format!("failed to transform CompileArkTS source {module_id}"))
    }

    fn output_code(
        &self,
        source: &str,
        transformed: Option<&rolldown_plugin::HookTransformOutput>,
        entry_info: &CompileModuleInfo,
    ) -> String {
        let code = transformed
            .and_then(|output| output.code.clone())
            .unwrap_or_else(|| compile_source_passthrough(source, &entry_info.source_ext));

        if matches!(
            entry_info.source_ext.as_str(),
            "ets" | "ts" | "js" | "mjs" | "cjs"
        ) {
            rewrite_compile_module_imports_to_ohm(
                &code,
                &entry_info.source_path,
                &self.cache_root,
                self.loader_json,
                self.config,
            )
        } else {
            code
        }
    }

    fn write_output_source(&self, entry_info: &CompileModuleInfo, code: String) -> Result<()> {
        if let Some(parent) = entry_info.ts_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        fs_err::write(&entry_info.ts_path, code).with_context(|| {
            format!(
                "failed to write transformed TS {}",
                entry_info.ts_path.display()
            )
        })
    }

    fn source_map_line(
        &self,
        module_id: &str,
        entry_info: &CompileModuleInfo,
        raw_map: &str,
    ) -> Result<String> {
        let map_value: serde_json::Value = serde_json::from_str(raw_map)
            .with_context(|| format!("failed to parse generated source map for {module_id}"))?;
        let mappings = map_value
            .get("mappings")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let file = Path::new(&entry_info.source_file)
            .file_name()
            .and_then(|file| file.to_str())
            .unwrap_or(entry_info.source_file.as_str());
        let key = entry_info.source_file_info.clone();
        let val = serde_json::json!({
            "version": 3,
            "file": file,
            "sourceRoot": "",
            "sources": [entry_info.source_path.to_string_lossy()],
            "names": [],
            "mappings": mappings,
            "entry-package-info": format!("{}|1.0.0", self.loader_json.module_name),
        });
        Ok(format!(
            "{{\"key\": {}, \"val\": {}}}",
            serde_json::to_string(&key)?,
            serde_json::to_string(&val)?
        ))
    }

    fn write_source_maps(&self, source_map_lines: &[String]) -> Result<()> {
        if source_map_lines.is_empty() {
            return Ok(());
        }
        fs_err::create_dir_all(&self.cache_root)?;
        fs_err::write(
            self.cache_root.join("sourceMaps.json"),
            source_map_lines.join("\n"),
        )?;
        Ok(())
    }

    fn populate_package_entries(&self, pkg_entries: &mut PackageEntryInfos) {
        if !self.loader_json.use_normalized_ohm_url {
            add_dynamic_import_package_entries(pkg_entries, self.loader_json, self.config);
        }
        add_native_package_entries(pkg_entries);
    }

    fn mode_dir(build_mode: BuildMode) -> &'static str {
        match build_mode {
            BuildMode::Debug => "debug",
            BuildMode::Release => "release",
        }
    }
}
