use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use arkdown_core::{CompileMode, OhosModuleType};
use arkdown_path::ArtifactPathSerializer;
use serde_json::{Map, Value};
use thiserror::Error;

use crate::layout::ModuleBuildLayout;

/// Failures while materializing Hvigor-compatible preview intermediates.
#[derive(Debug, Error)]
pub enum PreviewAssetError {
    #[error("ARKDOWN_PREVIEW_STAGE_ROUTER_PATHS: {path} stageRouterConfig.paths must be an array")]
    InvalidRouterPaths { path: PathBuf },
    #[error(
        "ARKDOWN_PREVIEW_STAGE_ROUTER_CONTENTS: {path} stageRouterConfig.contents must be an array"
    )]
    InvalidRouterContents { path: PathBuf },
    #[error(
    "ARKDOWN_PREVIEW_STAGE_ROUTER_LENGTH: the number of 'paths' ({path_count}) doesn't match that of 'contents' ({content_count}) in {path}"
  )]
    RouterLengthMismatch {
        path_count: usize,
        content_count: usize,
        path: PathBuf,
    },
    #[error(
        "ARKDOWN_PREVIEW_STAGE_ROUTER_PATH: {path} stageRouterConfig.paths entries must be strings"
    )]
    InvalidRouterPath { path: PathBuf },
    #[error(
    "ARKDOWN_PREVIEW_STAGE_ROUTER_CONTENT: {path} stageRouterConfig.contents entries must be strings"
  )]
    InvalidRouterContent { path: PathBuf },
    #[error("ARKDOWN_PREVIEW_MODULE_JSON: {path} does not contain a module object")]
    MissingModuleObject { path: PathBuf },
    #[error("ARKDOWN_PREVIEW_PAGES_NOT_FOUND: cannot find module.pages in {path}")]
    MissingPages { path: PathBuf },
    #[error("ARKDOWN_PREVIEW_PAGES_PROFILE: {path} must contain a JSON object")]
    InvalidPagesProfile { path: PathBuf },
    #[error("ARKDOWN_PREVIEW_MAIN_PAGES_SRC: stage router content for {path} has no src field")]
    MissingMainPagesSource { path: PathBuf },
    #[error("ARKDOWN_PREVIEW_MAIN_PAGES: {path} must contain a JSON object")]
    InvalidMainPages { path: PathBuf },
    #[error("{operation} {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("parse preview JSON {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("serialize preview JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("serialize preview path {path}: {message}")]
    PathSerialize { path: PathBuf, message: String },
}

/// Owns the filesystem actions performed between `buildPreviewerResource`
/// and `PreviewArkTS`.
///
/// Upstream: `PreviewUpdateAssets`, `CopyPreviewProfile`, and
/// `ReplacePreviewerPage` in hvigor-ohos-plugin 6.24.3.
pub struct PreviewAssetProcessor<'a> {
    project_root: &'a Path,
    module_root: &'a Path,
    module_name: &'a str,
    module_type: &'a OhosModuleType,
    product_name: &'a str,
    target_name: &'a str,
    compile_mode: &'a CompileMode,
    package_name: &'a str,
}

/// Stable preview inputs projected by the build coordinator.
pub struct PreviewAssetProcessorInput<'a> {
    pub project_root: &'a Path,
    pub module_root: &'a Path,
    pub module_name: &'a str,
    pub module_type: &'a OhosModuleType,
    pub product_name: &'a str,
    pub target_name: &'a str,
    pub compile_mode: &'a CompileMode,
    pub package_name: &'a str,
}

impl<'a> PreviewAssetProcessor<'a> {
    pub fn new(input: PreviewAssetProcessorInput<'a>) -> Self {
        Self {
            project_root: input.project_root,
            module_root: input.module_root,
            module_name: input.module_name,
            module_type: input.module_type,
            product_name: input.product_name,
            target_name: input.target_name,
            compile_mode: input.compile_mode,
            package_name: input.package_name,
        }
    }

    pub fn update_assets(&self) -> Result<(), PreviewAssetError> {
        let build_config_path = self.layout().preview_build_config_json();
        if !build_config_path.is_file() {
            tracing::debug!(
              path = %build_config_path.display(),
              "PreviewUpdateAssets build config is absent"
            );
            return Ok(());
        }
        let build_config = self.read_json(&build_config_path)?;
        let Some(stage_router) = build_config.get("stageRouterConfig") else {
            return Ok(());
        };
        let paths = stage_router
            .get("paths")
            .and_then(Value::as_array)
            .ok_or_else(|| PreviewAssetError::InvalidRouterPaths {
                path: build_config_path.clone(),
            })?;
        let contents = stage_router
            .get("contents")
            .and_then(Value::as_array)
            .ok_or_else(|| PreviewAssetError::InvalidRouterContents {
                path: build_config_path.clone(),
            })?;
        if paths.len() != contents.len() {
            return Err(PreviewAssetError::RouterLengthMismatch {
                path_count: paths.len(),
                content_count: contents.len(),
                path: build_config_path,
            });
        }

        let pages_file_name = self.preview_pages_file_name();
        for (path, content) in paths.iter().zip(contents) {
            let path = path
                .as_str()
                .ok_or_else(|| PreviewAssetError::InvalidRouterPath {
                    path: build_config_path.clone(),
                })?;
            let content =
                content
                    .as_str()
                    .ok_or_else(|| PreviewAssetError::InvalidRouterContent {
                        path: build_config_path.clone(),
                    })?;
            let output = self.resolve_stage_router_path(path);
            if output.file_name() == Some(OsStr::new("module.json")) {
                self.update_har_module_json(&output, content)?;
            } else if output.file_name() == Some(pages_file_name.as_os_str()) {
                self.update_main_pages(&output, content)?;
            }
        }
        Ok(())
    }

    pub fn copy_profile_for_incremental_resource(&self) -> Result<(), PreviewAssetError> {
        let source = self
            .module_root
            .join("src")
            .join(if self.target_name == "default" {
                "main"
            } else {
                self.target_name
            })
            .join("resources")
            .join("base")
            .join("profile");
        if !source.is_dir() {
            return Ok(());
        }
        let destination = self.layout().res_profile_dir();
        self.copy_directory(&source, &destination)
    }

    pub fn replace_page(
        &self,
        replacement: Option<&str>,
        compile_hsp_dependencies: bool,
        main_module: Option<&str>,
    ) -> Result<(), PreviewAssetError> {
        let layout = self.layout();
        let module_json_path = layout.res_module_json();
        if matches!(self.module_type, OhosModuleType::Shared) {
            let mut module_json = self.read_json(&module_json_path)?;
            let module = module_json
                .get_mut("module")
                .and_then(Value::as_object_mut)
                .ok_or_else(|| PreviewAssetError::MissingModuleObject {
                    path: module_json_path.clone(),
                })?;
            module.insert(
                "pages".to_string(),
                Value::String("$profile:main_pages".to_string()),
            );
            self.write_json(&module_json_path, &module_json)?;

            let source_main_pages = self
                .module_root
                .join("src")
                .join("main")
                .join("resources")
                .join("base")
                .join("profile")
                .join("main_pages.json");
            let preview_main_pages = layout.res_profile_dir().join("main_pages.json");
            if !source_main_pages.is_file() && !preview_main_pages.is_file() {
                self.write_json(
                    &preview_main_pages,
                    &serde_json::json!({ "src": ["pages/Index"] }),
                )?;
            }
        }

        if compile_hsp_dependencies
            && matches!(self.module_type, OhosModuleType::Shared)
            && main_module.is_some_and(|main| main != self.module_name)
        {
            return Ok(());
        }
        let Some(replacement) = replacement.filter(|page| !page.is_empty()) else {
            return Ok(());
        };
        let module_json = self.read_json(&module_json_path)?;
        let pages = module_json
            .pointer("/module/pages")
            .and_then(Value::as_str)
            .ok_or_else(|| PreviewAssetError::MissingPages {
                path: module_json_path.clone(),
            })?;
        let profile_name = pages.strip_prefix("$profile:").unwrap_or(pages);
        let profile_path = layout
            .res_profile_dir()
            .join(Self::json_file_name(profile_name));
        let mut profile = self.read_json(&profile_path)?;
        let profile_object =
            profile
                .as_object_mut()
                .ok_or_else(|| PreviewAssetError::InvalidPagesProfile {
                    path: profile_path.clone(),
                })?;
        profile_object.insert(
            "src".to_string(),
            Value::Array(vec![Value::String(replacement.to_string())]),
        );
        self.write_json(&profile_path, &profile)
    }

    pub fn persist_incremental_resource_parameters(
        &self,
        app_scope_resources: Option<&Path>,
        module_resources: &[PathBuf],
        har_resources: &[PathBuf],
        compile_module_names: &[String],
    ) -> Result<(), PreviewAssetError> {
        let app_resources = app_scope_resources
            .filter(|path| path.is_dir())
            .map(Self::serialize_path)
            .transpose()?
            .unwrap_or_default();
        let mut module_resource_map = Map::new();
        for (index, resource) in module_resources.iter().enumerate() {
            module_resource_map.insert(
                Self::serialize_path(resource)?,
                Value::String(Self::compiled_resource_folder("module", index)),
            );
        }
        let mut har_resource_map = Map::new();
        for (index, resource) in har_resources.iter().enumerate() {
            har_resource_map.insert(
                Self::serialize_path(resource)?,
                Value::String(Self::compiled_resource_folder("har", index)),
            );
        }
        let layout = self.layout();
        self.write_json(
            &layout.preview_build_param_json(),
            &serde_json::json!({
              "APP_RESOURCES": app_resources,
              "MODULE_RESOURCES": serde_json::to_string(&module_resource_map)?,
              "HAR_RESOURCES": serde_json::to_string(&har_resource_map)?,
              "PREVIEW_GENERATE_SOURCE_DIR": Self::serialize_path(
                &layout
                  .build_default()
                  .join("generated")
                  .join("r")
                  .join(self.target_name),
              )?,
              "PREVIEW_COMPILE_MODULE_NAMES": compile_module_names.join(","),
            }),
        )
    }

    fn compiled_resource_folder(prefix: &str, index: usize) -> String {
        if index == 0 {
            prefix.to_string()
        } else {
            format!("{prefix}{index}")
        }
    }

    fn layout(&self) -> ModuleBuildLayout<'_> {
        ModuleBuildLayout::for_preview_product(
            self.module_root,
            self.product_name,
            self.target_name,
        )
    }

    fn resolve_stage_router_path(&self, configured: &str) -> PathBuf {
        let path = PathBuf::from(configured);
        if path.is_absolute() {
            arkdown_path::lexical_normalize(&path)
        } else {
            arkdown_path::lexical_normalize(&self.project_root.join(path))
        }
    }

    fn update_har_module_json(
        &self,
        output: &Path,
        content: &str,
    ) -> Result<(), PreviewAssetError> {
        if !matches!(self.module_type, OhosModuleType::Har) {
            return Ok(());
        }
        if !output.is_file() {
            return self.write_text(output, content);
        }
        let incoming = self.parse_json(content, output)?;
        let mut existing = self.read_json(output)?;
        let existing_app = existing.get_mut("app").map(Value::take);
        let mut merged = incoming;
        if let (Some(app), Some(object)) = (existing_app, merged.as_object_mut()) {
            object.insert("app".to_string(), app);
        }
        if let Some(module) = merged.get_mut("module").and_then(Value::as_object_mut) {
            module.insert(
                "compileMode".to_string(),
                Value::String(
                    match self.compile_mode {
                        CompileMode::EsModule => "esmodule",
                        CompileMode::JsBundle => "jsbundle",
                    }
                    .to_string(),
                ),
            );
            module.insert(
                "packageName".to_string(),
                Value::String(self.package_name.to_string()),
            );
        }
        self.write_json(output, &merged)
    }

    fn update_main_pages(&self, output: &Path, content: &str) -> Result<(), PreviewAssetError> {
        let incoming = self.parse_json(content, output)?;
        let incoming_src = incoming.get("src").cloned().ok_or_else(|| {
            PreviewAssetError::MissingMainPagesSource {
                path: output.to_path_buf(),
            }
        })?;
        let mut existing = if output.is_file() {
            self.read_json(output)?
        } else {
            serde_json::json!({ "src": [] })
        };
        let object =
            existing
                .as_object_mut()
                .ok_or_else(|| PreviewAssetError::InvalidMainPages {
                    path: output.to_path_buf(),
                })?;
        object.insert("src".to_string(), incoming_src);
        self.write_json(output, &existing)
    }

    fn preview_pages_file_name(&self) -> std::ffi::OsString {
        let module_json_path = self.layout().res_module_json();
        let profile = self
            .read_json(&module_json_path)
            .ok()
            .and_then(|value| value.pointer("/module/pages")?.as_str().map(str::to_string))
            .map(|pages| pages.trim_start_matches("$profile:").to_string())
            .filter(|pages| !pages.is_empty())
            .unwrap_or_else(|| "main_pages".to_string());
        Self::json_file_name(&profile)
    }

    fn json_file_name(name: &str) -> std::ffi::OsString {
        let mut output = std::ffi::OsString::from(name);
        output.push(".json");
        output
    }

    fn read_json(&self, path: &Path) -> Result<Value, PreviewAssetError> {
        let text = fs_err::read_to_string(path).map_err(|source| PreviewAssetError::Io {
            operation: "read preview JSON",
            path: path.to_path_buf(),
            source,
        })?;
        self.parse_json(&text, path)
    }

    fn parse_json(&self, text: &str, path: &Path) -> Result<Value, PreviewAssetError> {
        serde_json::from_str(text)
            .or_else(|_| json5::from_str(text))
            .map_err(|error| PreviewAssetError::Parse {
                path: path.to_path_buf(),
                message: error.to_string(),
            })
    }

    fn write_json(&self, path: &Path, value: &Value) -> Result<(), PreviewAssetError> {
        let bytes = serde_json::to_vec(value)?;
        self.write_bytes(path, &bytes)
    }

    fn write_text(&self, path: &Path, value: &str) -> Result<(), PreviewAssetError> {
        self.write_bytes(path, value.as_bytes())
    }

    fn write_bytes(&self, path: &Path, value: &[u8]) -> Result<(), PreviewAssetError> {
        if let Some(parent) = path.parent() {
            fs_err::create_dir_all(parent).map_err(|source| PreviewAssetError::Io {
                operation: "create preview output directory",
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs_err::write(path, value).map_err(|source| PreviewAssetError::Io {
            operation: "write preview output",
            path: path.to_path_buf(),
            source,
        })
    }

    fn copy_directory(&self, source: &Path, destination: &Path) -> Result<(), PreviewAssetError> {
        fs_err::create_dir_all(destination).map_err(|source| PreviewAssetError::Io {
            operation: "create preview profile directory",
            path: destination.to_path_buf(),
            source,
        })?;
        let entries = fs_err::read_dir(source).map_err(|source_error| PreviewAssetError::Io {
            operation: "read preview profile directory",
            path: source.to_path_buf(),
            source: source_error,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source_error| PreviewAssetError::Io {
                operation: "read preview profile entry",
                path: source.to_path_buf(),
                source: source_error,
            })?;
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            let file_type = entry
                .file_type()
                .map_err(|source_error| PreviewAssetError::Io {
                    operation: "inspect preview profile entry",
                    path: source_path.clone(),
                    source: source_error,
                })?;
            if file_type.is_dir() {
                self.copy_directory(&source_path, &destination_path)?;
            } else {
                fs_err::copy(&source_path, &destination_path).map_err(|source| {
                    PreviewAssetError::Io {
                        operation: "copy preview profile",
                        path: destination_path,
                        source,
                    }
                })?;
            }
        }
        Ok(())
    }

    fn serialize_path(path: &Path) -> Result<String, PreviewAssetError> {
        ArtifactPathSerializer::path_to_slash_string(path).map_err(|error| {
            PreviewAssetError::PathSerialize {
                path: path.to_path_buf(),
                message: error.to_string(),
            }
        })
    }
}
