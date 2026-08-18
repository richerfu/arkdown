//! CompileArkTS module-source writer: hvigor ModuleMode cache output.

use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};

use arkdown_core::{ArkdownError, HvigorModuleSourceFile};
use arkdown_ets_transform::SourceClassifier;
use arkdown_path::{ArtifactPathSerializer, NormalizedOhmUrl};

use crate::chunk_module_request::{HvigorChunkImportMap, HvigorChunkModuleRequestProcessor};
use crate::plan::{BundleMockConfig, BundleRuntimePackageInfo};

use super::*;

pub(crate) struct HvigorModuleSourceWriter<'a> {
    pub(crate) bundle_name: &'a str,
    pub(crate) module_name: &'a str,
    pub(crate) project_root: &'a Path,
    pub(crate) module_path_map: &'a HvigorModulePathMap,
    pub(crate) runtime_packages: &'a BundleRuntimePackageInfo,
    pub(crate) bytecode_har_imports: HvigorBytecodeHarImports<'a>,
    pub(crate) use_normalized_ohm_url: bool,
    pub(crate) module_graph_facts: &'a ModuleGraphFacts,
    pub(crate) module_source_ids: Vec<String>,
    pub(crate) state: HvigorModuleSourceState,
    pub(crate) output_dir: PathBuf,
    pub(crate) processor: HvigorChunkModuleRequestProcessor,
    pub(crate) mock_config: Option<&'a BundleMockConfig>,
}

#[derive(Default)]
pub(crate) struct HvigorModuleSourceRecordIndex {
    pub(crate) by_module_id: FxHashMap<String, HvigorModuleSourceRecord>,
    pub(crate) by_canonical_module_id: FxHashMap<String, String>,
}

pub(crate) struct HvigorModuleSourceRecord {
    pub(crate) ownership: HvigorChunkSource,
    pub(crate) record_name: String,
}

pub(crate) struct HvigorModuleSourceWriteJob {
    pub(crate) module_id: String,
    pub(crate) path: PathBuf,
}

#[derive(Default)]
pub(crate) struct HvigorMockConfigRewrites {
    pub(crate) imports_by_module: FxHashMap<String, HvigorChunkImportMap>,
}

pub(crate) struct HvigorMockConfigSource {
    pub(crate) source: String,
    pub(crate) module_id: PathBuf,
}

impl<'a> HvigorModuleSourceWriter<'a> {
    pub(crate) fn new(
        bundle_name: &'a str,
        module_name: &'a str,
        project_root: &'a Path,
        module_path_map: &'a HvigorModulePathMap,
        runtime_packages: &'a BundleRuntimePackageInfo,
        byte_code_har_packages: &'a [String],
        use_normalized_ohm_url: bool,
        module_graph_facts: &'a ModuleGraphFacts,
        module_source_ids: Vec<String>,
        state: HvigorModuleSourceState,
        output_dir: PathBuf,
    ) -> Self {
        Self {
            bundle_name,
            module_name,
            project_root,
            module_path_map,
            runtime_packages,
            bytecode_har_imports: HvigorBytecodeHarImports::new(
                runtime_packages,
                byte_code_har_packages,
                use_normalized_ohm_url,
            ),
            use_normalized_ohm_url,
            module_graph_facts,
            module_source_ids,
            state,
            output_dir,
            processor: HvigorChunkModuleRequestProcessor::new(bundle_name, module_name),
            mock_config: None,
        }
    }

    pub(crate) fn with_mock_config(mut self, mock_config: Option<&'a BundleMockConfig>) -> Self {
        self.mock_config = mock_config;
        self
    }

    pub(crate) fn write(self) -> Result<Vec<HvigorModuleSourceFile>, ArkdownError> {
        let had_output_dir = self.output_dir.exists();
        if had_output_dir && !self.output_dir.is_dir() {
            fs_err::remove_file(&self.output_dir)
                .map_err(|e| ArkdownError::Bundler(e.to_string()))?;
        }
        if !self.output_dir.exists() {
            fs_err::create_dir_all(&self.output_dir)
                .map_err(|e| ArkdownError::Bundler(e.to_string()))?;
        }

        let mut module_ids = self
            .module_source_ids
            .iter()
            .filter(|module_id| self.state.sources.contains_key(module_id.as_str()))
            .filter(|module_id| HvigorModuleSourceId::new(module_id).should_compile())
            .cloned()
            .collect::<Vec<_>>();
        module_ids.dedup();

        let module_records = self.module_source_record_index(&module_ids)?;
        let mock_rewrites = self.prepare_mock_config(&module_records)?;
        let jobs = module_ids
            .iter()
            .filter_map(|module_id| {
                self.module_source_write_job(module_id, &module_records)
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut destinations = FxHashMap::<PathBuf, String>::default();
        for job in &jobs {
            if let Some(existing) = destinations.insert(job.path.clone(), job.module_id.clone()) {
                return Err(ArkdownError::Bundler(format!(
                    "CompileArkTS module sources '{existing}' and '{}' resolve to the same output path {}",
                    job.module_id,
                    job.path.display()
                )));
            }
        }
        let files = jobs
            .par_iter()
            .map(|job| self.write_module_source(job, &module_records, &mock_rewrites))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let active_paths = files
            .iter()
            .map(|file| file.path.clone())
            .collect::<FxHashSet<_>>();
        if had_output_dir {
            self.remove_stale_output_files(&active_paths)?;
        }
        Ok(files)
    }

    /// Mirror ets-loader `ModuleSourceFile` mock processing at its ModuleMode
    /// boundary. Runtime imports continue to reference the real OHM record;
    /// `mock-config.json` maps that record to the configured mock. Static
    /// imports made by a mock source receive the SDK `.origin` suffix and a
    /// reverse entry so hamock can reach the unmocked implementation.
    pub(crate) fn prepare_mock_config(
        &self,
        module_records: &HvigorModuleSourceRecordIndex,
    ) -> Result<HvigorMockConfigRewrites, ArkdownError> {
        let Some(options) = self.mock_config else {
            return Ok(HvigorMockConfigRewrites::default());
        };
        let Some(config_path) = options
            .process
            .then_some(options.user_config_path.as_ref())
            .flatten()
        else {
            self.remove_mock_config_cache(options)?;
            return Ok(HvigorMockConfigRewrites::default());
        };

        let user_content = fs_err::read_to_string(config_path)
            .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
        let user_document =
            json5::from_str::<serde_json::Value>(&user_content).map_err(|error| {
                ArkdownError::Bundler(format!(
                    "invalid mock config '{}': {error}",
                    config_path.display()
                ))
            })?;
        let user_entries = user_document.as_object().ok_or_else(|| {
            ArkdownError::Bundler(format!(
                "mock config '{}' must contain a JSON object",
                config_path.display()
            ))
        })?;
        let module_root = self.module_path_map.current_module_root();
        let mut configured_sources = IndexMap::<String, HvigorMockConfigSource>::new();
        let mut mock_module_ids = FxHashSet::default();
        for (key, entry) in user_entries {
            let Some(source) = entry.get("source").and_then(serde_json::Value::as_str) else {
                continue;
            };
            if source.is_empty() {
                continue;
            }
            let configured_path = PathBuf::from(source);
            let module_id = if configured_path.has_root() {
                configured_path
            } else {
                module_root.join(configured_path)
            };
            let module_id = arkdown_path::canonicalize_or_owned(&module_id);
            if !module_id.is_file() {
                return Err(ArkdownError::Bundler(format!(
                    "mock source '{}' configured by '{}' does not exist",
                    module_id.display(),
                    config_path.display()
                )));
            }
            mock_module_ids.insert(module_id.clone());
            configured_sources.insert(
                key.clone(),
                HvigorMockConfigSource {
                    source: source.to_string(),
                    module_id,
                },
            );
        }

        let mut rewrites = HvigorMockConfigRewrites::default();
        let mut transformed_entries = serde_json::Map::new();
        // `collectMockConfigInfo()` eagerly converts dependency keys through
        // `getMockConfigKey2ModuleInfoMap`, even when no source currently
        // imports that dependency. Preserve that observable config output.
        for (key, configured) in &configured_sources {
            let Some(real_module_id) = self.mock_dependency_module_id(key, options) else {
                continue;
            };
            let real_module_id = real_module_id.to_string_lossy();
            let Some(real_ohm_url) =
                self.ohm_url_for_resolved_id(&real_module_id, module_records)?
            else {
                continue;
            };
            let mock_module_id = configured.module_id.to_string_lossy();
            let Some(mock_ohm_url) =
                self.ohm_url_for_resolved_id(&mock_module_id, module_records)?
            else {
                continue;
            };
            transformed_entries
                .entry(real_ohm_url)
                .or_insert_with(|| serde_json::json!({ "source": mock_ohm_url }));
        }
        let mut module_ids = self.module_source_ids.clone();
        module_ids.sort();
        module_ids.dedup();
        for module_id in module_ids {
            let Some(source) = self.state.sources.get(&module_id) else {
                continue;
            };
            let (imports, _) = self.imports_for_module(&module_id, module_records)?;
            let canonical_module_id = arkdown_path::canonicalize_or_owned(Path::new(&module_id));
            let is_mock_source = mock_module_ids.contains(&canonical_module_id);
            let requests = HvigorModuleRequestSpecifiers::new(source.code.as_ref(), &module_id)
                .collect_literals();
            for request in requests {
                let Some(ohm_url) = imports.get(&request.specifier) else {
                    continue;
                };
                let configured = configured_sources.get(&request.specifier).or_else(|| {
                    self.local_mock_config_key(&module_id, &request.specifier, options)
                        .and_then(|key| configured_sources.get(&key))
                });
                if let Some(configured) = configured {
                    let mock_module_id = configured.module_id.to_string_lossy();
                    let mock_ohm_url = self
                        .ohm_url_for_resolved_id(&mock_module_id, module_records)?
                        .ok_or_else(|| {
                            ArkdownError::Bundler(format!(
                                "cannot derive OHM URL for mock source '{}'",
                                configured.source
                            ))
                        })?;
                    transformed_entries
                        .entry(ohm_url.clone())
                        .or_insert_with(|| serde_json::json!({ "source": mock_ohm_url }));
                }
                if is_mock_source
                    && !request.is_dynamic
                    && !SourceClassifier::classify(&module_id).is_javascript_runtime()
                {
                    let origin_url = format!("{ohm_url}.origin");
                    rewrites
                        .imports_by_module
                        .entry(module_id.clone())
                        .or_default()
                        .insert(request.specifier.clone(), origin_url.clone());
                    transformed_entries
                        .entry(origin_url)
                        .or_insert_with(|| serde_json::json!({ "source": ohm_url }));
                }
            }
        }
        self.write_mock_config_artifacts(
            options,
            &user_document,
            serde_json::Value::Object(transformed_entries),
        )?;
        Ok(rewrites)
    }

    pub(crate) fn mock_dependency_module_id(
        &self,
        key: &str,
        options: &BundleMockConfig,
    ) -> Option<PathBuf> {
        let mut components = key.split(['/', '\\']);
        let first = components.next()?;
        if first.starts_with('.') {
            return None;
        }
        let dependency_name = if first.starts_with('@') {
            let package = components.next()?;
            format!("{first}/{package}")
        } else {
            first.to_string()
        };
        let dependency = options
            .dependencies
            .iter()
            .rev()
            .find(|dependency| dependency.dependency_name == dependency_name)?;
        let suffix = components.fold(PathBuf::new(), |path, component| path.join(component));
        Some(if suffix.as_os_str().is_empty() {
            dependency.main_file_path.clone()
        } else {
            dependency.package_root.join(suffix)
        })
    }

    pub(crate) fn local_mock_config_key(
        &self,
        module_id: &str,
        specifier: &str,
        options: &BundleMockConfig,
    ) -> Option<String> {
        let resolved_id = self.state.resolved_imports.get(module_id)?.get(specifier)?;
        let resolved_path = arkdown_path::canonicalize_or_owned(Path::new(resolved_id));
        let source_root = arkdown_path::canonicalize_or_owned(&options.ets_source_root);
        let relative = resolved_path.strip_prefix(source_root).ok()?;
        Some(ArtifactPathSerializer::rel_to_slash(relative))
    }

    pub(crate) fn remove_mock_config_cache(
        &self,
        options: &BundleMockConfig,
    ) -> Result<(), ArkdownError> {
        for path in [&options.transformed_cache_path, &options.user_cache_path] {
            match fs_err::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(ArkdownError::Bundler(error.to_string())),
            }
        }
        Ok(())
    }

    pub(crate) fn write_mock_config_artifacts(
        &self,
        options: &BundleMockConfig,
        user_document: &serde_json::Value,
        transformed_document: serde_json::Value,
    ) -> Result<(), ArkdownError> {
        for path in [
            &options.transformed_output_path,
            &options.transformed_cache_path,
            &options.user_cache_path,
        ] {
            if let Some(parent) = path.parent() {
                fs_err::create_dir_all(parent)
                    .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
            }
        }

        let serialized_user = serde_json::to_vec(user_document)
            .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
        let cached_user = fs_err::read_to_string(&options.user_cache_path)
            .ok()
            .and_then(|content| json5::from_str::<serde_json::Value>(&content).ok())
            .and_then(|document| serde_json::to_vec(&document).ok());
        let cache_is_usable = options.transformed_cache_path.is_file()
            && cached_user.as_deref() == Some(serialized_user.as_slice());

        if !cache_is_usable {
            let transformed = serde_json::to_vec(&transformed_document)
                .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
            fs_err::write(&options.transformed_output_path, &transformed)
                .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
            fs_err::write(&options.transformed_cache_path, transformed)
                .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
            let user_config_path = options.user_config_path.as_ref().ok_or_else(|| {
                ArkdownError::Bundler("active mock processing has no user config path".to_string())
            })?;
            fs_err::copy(user_config_path, &options.user_cache_path)
                .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
            return Ok(());
        }

        let cached_content = fs_err::read_to_string(&options.transformed_cache_path)
            .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
        let mut cached_document = json5::from_str::<serde_json::Value>(&cached_content)
            .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
        let cached_entries = cached_document.as_object_mut().ok_or_else(|| {
            ArkdownError::Bundler(format!(
                "cached mock config '{}' must contain a JSON object",
                options.transformed_cache_path.display()
            ))
        })?;
        let transformed_entries = transformed_document.as_object().ok_or_else(|| {
            ArkdownError::Bundler("transformed mock config must contain a JSON object".to_string())
        })?;
        let mut changed = false;
        for (key, value) in transformed_entries {
            if !cached_entries.contains_key(key) {
                cached_entries.insert(key.clone(), value.clone());
                changed = true;
            }
        }
        if changed {
            let merged = serde_json::to_vec(&cached_document)
                .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
            fs_err::write(&options.transformed_output_path, &merged)
                .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
            fs_err::write(&options.transformed_cache_path, merged)
                .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
        } else {
            fs_err::copy(
                &options.transformed_cache_path,
                &options.transformed_output_path,
            )
            .map_err(|error| ArkdownError::Bundler(error.to_string()))?;
        }
        Ok(())
    }

    pub(crate) fn module_source_write_job(
        &self,
        module_id: &str,
        module_records: &HvigorModuleSourceRecordIndex,
    ) -> Result<Option<HvigorModuleSourceWriteJob>, ArkdownError> {
        if !self.state.sources.contains_key(module_id) {
            return Ok(None);
        }
        let Some(module_record) = module_records.by_module_id.get(module_id) else {
            return Ok(None);
        };
        let ownership = &module_record.ownership;
        let emits_javascript = self.state.javascript_module_ids.contains(module_id);
        let path = HvigorModuleSourceOutputPath::new(
            &self.output_dir,
            self.project_root,
            ownership,
            &ownership.module_name,
            &ownership.source_file,
            module_id,
            emits_javascript,
        )
        .path()?;
        Ok(Some(HvigorModuleSourceWriteJob {
            module_id: module_id.to_string(),
            path,
        }))
    }

    pub(crate) fn write_module_source(
        &self,
        job: &HvigorModuleSourceWriteJob,
        module_records: &HvigorModuleSourceRecordIndex,
        mock_rewrites: &HvigorMockConfigRewrites,
    ) -> Result<Option<HvigorModuleSourceFile>, ArkdownError> {
        let module_id = job.module_id.as_str();
        let Some(source) = self.state.sources.get(module_id) else {
            return Ok(None);
        };
        let Some(module_record) = module_records.by_module_id.get(module_id) else {
            return Ok(None);
        };
        let ownership = &module_record.ownership;
        let (mut imports, imported_record_names) =
            self.imports_for_module(module_id, module_records)?;
        if let Some(overrides) = mock_rewrites.imports_by_module.get(module_id) {
            imports.extend(
                overrides
                    .iter()
                    .map(|(specifier, request)| (specifier.clone(), request.clone())),
            );
        }
        let code = HvigorModuleSourceImportRewriter::new(source.code.as_ref(), module_id, &imports)
            .rewrite();
        let emits_javascript = self.state.javascript_module_ids.contains(module_id);
        if let Some(parent) = job.path.parent() {
            fs_err::create_dir_all(parent).map_err(|e| ArkdownError::Bundler(e.to_string()))?;
        }
        let code = Self::finalize_source_text(code.into_owned(), emits_javascript);
        self.write_file(&job.path, code.as_bytes())?;
        let origin_source_lang = HvigorModuleSourceId::new(module_id)
            .origin_source_lang()
            .to_string();
        let entry_package = self.runtime_packages.packages.get(self.module_name);
        let current_package_identity = self.module_path_map.current_package_identity();
        let entry_package_name = entry_package
            .map(|context| context.package_name.as_str())
            .filter(|name| !name.is_empty())
            .or_else(|| current_package_identity.map(|(name, _)| name))
            .unwrap_or(self.module_name);
        let entry_package_version = entry_package
            .map(|context| context.version.as_str())
            .filter(|version| !version.is_empty())
            .or_else(|| current_package_identity.map(|(_, version)| version))
            .unwrap_or_default();
        Ok(Some(HvigorModuleSourceFile {
            module_id: module_id.to_string(),
            path: job.path.clone(),
            record_name: module_record.record_name.clone(),
            source_file: ownership.file_info_source_file(
                entry_package_name,
                entry_package_version,
                self.module_path_map.has_dependency_package_info(module_id),
                emits_javascript,
            ),
            package_name: Some(self.file_info_package_name(module_id, ownership)?),
            is_commonjs: self
                .module_graph_facts
                .full_module_is_commonjs
                .get(module_id)
                .copied()
                .unwrap_or(false),
            is_shared_module: self.state.shared_module_ids.contains(module_id),
            origin_source_lang,
            imported_record_names,
        }))
    }

    pub(crate) fn finalize_source_text(mut code: String, emits_javascript: bool) -> String {
        if !emits_javascript && !code.ends_with('\n') {
            code.push('\n');
        }
        code
    }

    pub(crate) fn write_file(&self, path: &Path, contents: &[u8]) -> Result<(), ArkdownError> {
        match fs_err::metadata(path) {
            Ok(metadata) if metadata.is_dir() => {
                fs_err::remove_dir_all(path).map_err(|e| ArkdownError::Bundler(e.to_string()))?;
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(ArkdownError::Bundler(error.to_string())),
        }

        fs_err::write(path, contents).map_err(|e| ArkdownError::Bundler(e.to_string()))
    }

    pub(crate) fn remove_stale_output_files(
        &self,
        active_paths: &FxHashSet<PathBuf>,
    ) -> Result<(), ArkdownError> {
        if !self.output_dir.is_dir() {
            return Ok(());
        }

        let mut stack = vec![self.output_dir.clone()];
        let mut dirs = Vec::new();
        while let Some(dir) = stack.pop() {
            dirs.push(dir.clone());
            for entry in fs_err::read_dir(&dir).map_err(|e| ArkdownError::Bundler(e.to_string()))? {
                let entry = entry.map_err(|e| ArkdownError::Bundler(e.to_string()))?;
                let file_type = entry
                    .file_type()
                    .map_err(|e| ArkdownError::Bundler(e.to_string()))?;
                let path = entry.path();
                if file_type.is_dir() {
                    stack.push(path);
                    continue;
                }
                let is_stale_module_source =
                    file_type.is_file() && !active_paths.contains(&path) && {
                        let source_kind = SourceClassifier::classify_path(&path);
                        source_kind.is_ets_transform_input() || source_kind.is_javascript_runtime()
                    };
                if is_stale_module_source {
                    fs_err::remove_file(&path).map_err(|e| ArkdownError::Bundler(e.to_string()))?;
                }
            }
        }

        for dir in dirs.into_iter().rev() {
            if dir == self.output_dir {
                continue;
            }
            match fs_err::remove_dir(&dir) {
                Ok(()) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::DirectoryNotEmpty | ErrorKind::NotFound
                    ) => {}
                Err(error) => return Err(ArkdownError::Bundler(error.to_string())),
            }
        }
        Ok(())
    }

    pub(crate) fn module_source_record_index(
        &self,
        module_ids: &[String],
    ) -> Result<HvigorModuleSourceRecordIndex, ArkdownError> {
        let mut index = HvigorModuleSourceRecordIndex::default();
        for module_id in module_ids {
            let ownership = self.module_path_map.source_file_for_entry(module_id)?;
            if ownership.module_name.is_empty() {
                continue;
            }
            let record_name = if let Some(record_name) =
                self.module_path_map.module_meta_info_ohm_url(module_id)
            {
                record_name.to_string()
            } else if self.use_normalized_ohm_url {
                ownership.normalized_record_name(self.runtime_packages)?
            } else {
                self.module_path_map.non_normalized_record_name(
                    self.project_root,
                    module_id,
                    self.bundle_name,
                    self.module_name,
                    &ownership,
                )?
            };
            if let Some(canonical_module_id) = Self::canonical_module_id(module_id) {
                index
                    .by_canonical_module_id
                    .insert(canonical_module_id, module_id.clone());
            }
            index.by_module_id.insert(
                module_id.clone(),
                HvigorModuleSourceRecord {
                    ownership,
                    record_name,
                },
            );
        }
        Ok(index)
    }

    pub(crate) fn canonical_module_id(module_id: &str) -> Option<String> {
        let path = Path::new(module_id);
        path.has_root().then(|| {
            HvigorModulePathMap::canonical_root(path)
                .to_string_lossy()
                .into_owned()
        })
    }

    pub(crate) fn record_for_resolved_id<'b>(
        module_records: &'b HvigorModuleSourceRecordIndex,
        resolved_id: &str,
    ) -> Option<&'b HvigorModuleSourceRecord> {
        module_records.by_module_id.get(resolved_id).or_else(|| {
            Self::canonical_module_id(resolved_id)
                .and_then(|canonical| module_records.by_canonical_module_id.get(&canonical))
                .and_then(|module_id| module_records.by_module_id.get(module_id))
        })
    }

    pub(crate) fn file_info_package_name(
        &self,
        module_id: &str,
        ownership: &HvigorChunkSource,
    ) -> Result<String, ArkdownError> {
        // The generated ResourceTable carries the owning module name in its
        // fixed Ohm URL (e.g. `com.example.arkdown.test/entry/build/
        // generated/r/ResourceTable`) even for the ohosTest module.
        if let Some(ohm_url) = self.module_path_map.module_meta_info_ohm_url(module_id) {
            if let Some((_, module)) = ohm_url.split_once('/') {
                if let Some(module) = module.split('/').next() {
                    return Ok(module.to_string());
                }
            }
        }
        match &ownership.kind {
            // `ModuleMode.processModuleInfos` uses `getPkgModulesFilePkgName`
            // for ordinary OHPM packages. The value is the physical OHPM
            // record root (for example `pkg_modules/.ohpm/foo@1/pkg_modules/foo`),
            // not the logical package name exposed in pkgContextInfo.
            //
            // Evidence: DevEco Studio 6.24.3 ets-loader
            // `module/module_mode.js::processModuleInfos`.
            HvigorChunkSourceKind::PackageModule(package) => self
                .module_path_map
                .package_record_for_path(self.project_root, &package.package_root)
                .ok_or_else(|| {
                    ArkdownError::Bundler(format!(
                        "cannot derive hvigor packageName for module '{}' rooted at {}",
                        module_id,
                        package.package_root.display()
                    ))
                }),
            HvigorChunkSourceKind::SourceModule
            | HvigorChunkSourceKind::LocalDependencyModule(_) => {
                // Files under the current module root carry the entry module
                // name (hvigor `pkgName@<module>` for the `ohosTest` template
                // tree uses the test module name `entry_test`).
                if arkdown_path::AbsPath::canonicalize(Path::new(module_id))
                    .map(|path| path.starts_with(self.module_path_map.current_module_root()))
                    .unwrap_or(false)
                {
                    Ok(self.module_name.to_string())
                } else {
                    Ok(ownership.module_name.clone())
                }
            }
        }
    }

    pub(crate) fn imports_for_module(
        &self,
        module_id: &str,
        module_records: &HvigorModuleSourceRecordIndex,
    ) -> Result<(HvigorChunkImportMap, Vec<String>), ArkdownError> {
        let mut imports = HvigorChunkImportMap::default();
        let mut imported_record_names = BTreeSet::new();
        let mut seen_specifiers = FxHashSet::default();

        if let Some(resolved_imports) = self.state.resolved_imports.get(module_id) {
            for (specifier, resolved_id) in resolved_imports {
                seen_specifiers.insert(specifier.clone());
                self.record_import(
                    module_id,
                    specifier,
                    resolved_id,
                    &mut imports,
                    &mut imported_record_names,
                    module_records,
                )?;
            }
        }
        if let Some(source) = self.state.sources.get(module_id) {
            for specifier in
                HvigorModuleRequestSpecifiers::new(source.code.as_ref(), module_id).collect()
            {
                if !seen_specifiers.insert(specifier.clone()) {
                    continue;
                }
                self.record_import(
                    module_id,
                    &specifier,
                    &specifier,
                    &mut imports,
                    &mut imported_record_names,
                    module_records,
                )?;
            }
        }

        Ok((imports, imported_record_names.into_iter().collect()))
    }

    pub(crate) fn record_import(
        &self,
        _importer: &str,
        specifier: &str,
        resolved_id: &str,
        imports: &mut HvigorChunkImportMap,
        imported_record_names: &mut BTreeSet<String>,
        module_records: &HvigorModuleSourceRecordIndex,
    ) -> Result<(), ArkdownError> {
        if let Some(ohm_url) = self.bytecode_har_imports.ohm_url_for_request(specifier)? {
            let record_name = NormalizedOhmUrl::parse(&ohm_url)
                .and_then(|url| url.record_name())
                .ok_or_else(|| {
                    ArkdownError::Bundler(format!(
                        "bytecode HAR import '{}' produced invalid normalized OHM URL '{}'",
                        specifier, ohm_url
                    ))
                })?;
            imported_record_names.insert(record_name);
            imports.insert(specifier.to_string(), ohm_url);
            return Ok(());
        }
        let sdk_request = self.processor.process_request(specifier);
        if sdk_request != specifier {
            imports.insert(specifier.to_string(), sdk_request);
            return Ok(());
        }
        let resolved_ohm_url = self.ohm_url_for_resolved_id(resolved_id, module_records)?;
        let specifier_ohm_url = if resolved_ohm_url.is_some() {
            None
        } else {
            self.ohm_url_for_resolved_id(specifier, module_records)?
        };
        if let Some(ohm_url) = resolved_ohm_url.or(specifier_ohm_url) {
            imports.insert(specifier.to_string(), ohm_url);
        }
        if let Some(record) = Self::record_for_resolved_id(module_records, resolved_id) {
            imported_record_names.insert(record.record_name.clone());
        }
        Ok(())
    }

    pub(crate) fn ohm_url_for_resolved_id(
        &self,
        resolved_id: &str,
        module_records: &HvigorModuleSourceRecordIndex,
    ) -> Result<Option<String>, ArkdownError> {
        if arkdown_path::is_ohm_runtime_specifier(resolved_id) {
            return Ok(Some(resolved_id.to_string()));
        }

        let path = Path::new(resolved_id);
        if !path.has_root() {
            return Ok(None);
        }
        if let Some(record) = Self::record_for_resolved_id(module_records, resolved_id) {
            return self.ohm_url_for_module_record(record).map(Some);
        }
        let ownership = self.module_path_map.source_file_for_entry(resolved_id)?;
        if ownership.module_name.is_empty() {
            return Ok(None);
        }
        if self.use_normalized_ohm_url {
            return ownership
                .to_normalized_request(self.runtime_packages, self.module_name)
                .map(Some);
        }
        let record_name = self.module_path_map.non_normalized_record_name(
            self.project_root,
            resolved_id,
            self.bundle_name,
            self.module_name,
            &ownership,
        )?;
        Ok(Some(self.legacy_ohm_url(&ownership, &record_name)))
    }

    pub(crate) fn ohm_url_for_module_record(
        &self,
        record: &HvigorModuleSourceRecord,
    ) -> Result<String, ArkdownError> {
        if self.use_normalized_ohm_url {
            return record
                .ownership
                .to_normalized_request(self.runtime_packages, self.module_name);
        }
        Ok(self.legacy_ohm_url(&record.ownership, &record.record_name))
    }

    pub(crate) fn legacy_ohm_url(
        &self,
        ownership: &HvigorChunkSource,
        record_name: &str,
    ) -> String {
        if record_name.starts_with('@') {
            return record_name.to_string();
        }
        match &ownership.kind {
            HvigorChunkSourceKind::PackageModule(_) => format!("@package:{record_name}"),
            HvigorChunkSourceKind::SourceModule
            | HvigorChunkSourceKind::LocalDependencyModule(_) => {
                format!("@bundle:{record_name}")
            }
        }
    }
}
