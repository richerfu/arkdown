//! Hvigor OHOS-context NAPI read/write adapter.

use super::*;

impl<'a> RuntimeOhosContextNapiAdapter<'a> {
  pub(super) fn new(
    runtime: &'a mut RuntimeBuildScriptSession,
    request: RuntimeOhosContextReadRequest,
  ) -> Self {
    Self {
      runtime,
      root: request.root.map(PathBuf::from),
      argument: request.argument,
      build_mode: request.build_mode,
      debug_line: request.debug_line,
      path_options: request.path_options.into(),
    }
  }

  pub(super) fn read(&mut self, operation: RuntimeOhosContextReadOperation) -> Result<Value> {
    if let Some(field) = operation.package_field() {
      return Ok(self.package_field(field).unwrap_or(Value::Null));
    }
    match operation {
      RuntimeOhosContextReadOperation::ProjectName => Ok(Value::String(self.project_name()?)),
      RuntimeOhosContextReadOperation::ProjectPath => {
        self.path_value(self.runtime.session.project_model().project_root.clone())
      }
      RuntimeOhosContextReadOperation::BuildRootPath => self.project_paths_value("buildRootPath"),
      RuntimeOhosContextReadOperation::BuildProductOutputPath => {
        self.project_paths_value("buildProductOutputPath")
      }
      RuntimeOhosContextReadOperation::BuildMode => Ok(Value::String(self.build_mode.clone())),
      RuntimeOhosContextReadOperation::BuildProfile => Ok(self.build_profile()),
      RuntimeOhosContextReadOperation::CurrentProduct => self.current_product(),
      RuntimeOhosContextReadOperation::AppJson => Ok(self.runtime.session.project_app_json()),
      RuntimeOhosContextReadOperation::SdkDetails => self.sdk_details(),
      RuntimeOhosContextReadOperation::DebugLine => Ok(Value::Bool(self.debug_line)),
      RuntimeOhosContextReadOperation::FastBuildApp => Ok(Value::Bool(
        self
          .runtime
          .session
          .project_build_profile()
          .pointer("/app/buildOption/packOptions/fastBuildApp")
          .and_then(Value::as_bool)
          .unwrap_or(false),
      )),
      RuntimeOhosContextReadOperation::AppWithSignedPkg => Ok(Value::Bool(
        self
          .runtime
          .session
          .project_build_profile()
          .pointer("/app/buildOption/packOptions/appWithSignedPkg")
          .and_then(Value::as_bool)
          .unwrap_or(false),
      )),
      RuntimeOhosContextReadOperation::DependencyInfo => {
        self.dependency_value("ohpmDependencyInfo")
      }
      RuntimeOhosContextReadOperation::RemoteHspDependencyInfo => {
        let field = if self.argument.as_ref().and_then(Value::as_bool) == Some(true) {
          "signedRemoteHspDependencyInfo"
        } else {
          "remoteHspDependencyInfo"
        };
        self.dependency_value(field)
      }
      RuntimeOhosContextReadOperation::AllModuleNameHash => self
        .runtime
        .session
        .all_module_name_hash()
        .map(Value::String)
        .map_err(Self::context_error),
      RuntimeOhosContextReadOperation::ModuleName => Ok(Value::String(self.module_name()?)),
      RuntimeOhosContextReadOperation::ModulePath
      | RuntimeOhosContextReadOperation::BuildProductRootPath => {
        self.path_value(self.module_root()?.to_path_buf())
      }
      RuntimeOhosContextReadOperation::ModuleType => Ok(Value::String(self.module_type()?)),
      RuntimeOhosContextReadOperation::Targets => self.targets(),
      RuntimeOhosContextReadOperation::TargetArtifactNames => self.target_artifact_names(),
      RuntimeOhosContextReadOperation::TargetArtifactName => self.target_artifact_name(),
      RuntimeOhosContextReadOperation::ModuleJson => Ok(
        self
          .runtime
          .session
          .module_json(self.module_root()?)
          .unwrap_or(Value::Null),
      ),
      RuntimeOhosContextReadOperation::Dependencies
      | RuntimeOhosContextReadOperation::DevDependencies
      | RuntimeOhosContextReadOperation::DynamicDependencies
      | RuntimeOhosContextReadOperation::Overrides
      | RuntimeOhosContextReadOperation::Version => unreachable!("package fields handled above"),
    }
  }

  fn project_name(&self) -> Result<String> {
    self
      .runtime
      .session
      .project_model()
      .project_root
      .file_name()
      .and_then(|name| name.to_str())
      .map(str::to_string)
      .ok_or_else(|| Error::new(Status::InvalidArg, "project root has no UTF-8 file name"))
  }

  fn module_root(&self) -> Result<&PathBuf> {
    self.root.as_ref().ok_or_else(|| {
      Error::new(
        Status::InvalidArg,
        "module OHOS context operation requires a module root",
      )
    })
  }

  fn module_descriptor(&self) -> Result<Value> {
    let root = self.module_root()?;
    self.runtime.session.module_descriptor(root).ok_or_else(|| {
      Error::new(
        Status::InvalidArg,
        format!(
          "runtime session has no module descriptor for {}",
          root.display()
        ),
      )
    })
  }

  fn module_name(&self) -> Result<String> {
    self
      .module_descriptor()?
      .get("name")
      .and_then(Value::as_str)
      .map(str::to_string)
      .ok_or_else(|| Error::new(Status::InvalidArg, "module descriptor has no name"))
  }

  fn module_type(&self) -> Result<String> {
    let kind = self
      .module_descriptor()?
      .get("kind")
      .and_then(Value::as_str)
      .unwrap_or_default()
      .to_string();
    Ok(match kind.as_str() {
      "hap" => "entry".to_string(),
      "har" => "har".to_string(),
      "hsp" => "shared".to_string(),
      _ => kind,
    })
  }

  fn build_profile(&self) -> Value {
    self.root.as_ref().map_or_else(
      || self.runtime.session.project_build_profile(),
      |root| {
        self
          .runtime
          .session
          .module_build_profile(root)
          .unwrap_or(Value::Null)
      },
    )
  }

  fn package_field(&self, field: &str) -> Option<Value> {
    self.root.as_ref().map_or_else(
      || self.runtime.session.project_package_field(field),
      |root| self.runtime.session.module_package_field(root, field),
    )
  }

  fn project_paths_value(&self, field: &str) -> Result<Value> {
    let paths = self
      .runtime
      .session
      .project_context_paths(&self.path_options)
      .map_err(Self::context_error)?;
    let value = serde_json::to_value(paths)
      .map_err(|error| Error::new(Status::GenericFailure, error.to_string()))?;
    Ok(value.get(field).cloned().unwrap_or(Value::Null))
  }

  fn current_product(&self) -> Result<Value> {
    let product = self.runtime.session.selected_product();
    Ok(serde_json::json!({
      "__arkdownFacade": "product",
      "productName": product.get("name").cloned().unwrap_or(Value::Null),
      "bundleType": product.get("bundleType").cloned().unwrap_or(Value::String("app".to_string())),
      "bundleName": self.runtime.session.selected_product_bundle_name(),
      "outputArtifactName": product.pointer("/output/artifactName").cloned().unwrap_or(Value::Null),
    }))
  }

  fn sdk_details(&mut self) -> Result<Value> {
    let details = self
      .runtime
      .session
      .sdk_details(&self.runtime.sdk_config)
      .map_err(BuildScriptRuntimeModelNapiAdapter::sdk_error)?;
    Ok(serde_json::json!({
      "__arkdownFacade": "sdkDetails",
      "isOhos": details.is_ohos,
      "sdkDir": details.sdk_dir,
      "sdkVersion": details.sdk_version,
      "etsComponentVersion": details.ets_component_version,
      "etsComponentReleaseType": details.ets_component_release_type,
    }))
  }

  fn dependency_value(&mut self, field: &str) -> Result<Value> {
    let context = self
      .runtime
      .session
      .dependency_context(self.root.as_deref())
      .map_err(Self::context_error)?;
    let value = serde_json::to_value(context)
      .map_err(|error| Error::new(Status::GenericFailure, error.to_string()))?;
    Ok(value.get(field).cloned().unwrap_or(Value::Null))
  }

  fn targets(&self) -> Result<Value> {
    let root = self.module_root()?;
    let plan = self.runtime.session.module_plan(root).ok_or_else(|| {
      Error::new(
        Status::InvalidArg,
        format!(
          "runtime plan has no module target plan for {}",
          root.display()
        ),
      )
    })?;
    let product = self.runtime.session.selected_product();
    let bundle_name = self.runtime.session.selected_product_bundle_name();
    let mut targets = Vec::with_capacity(plan.selected_targets.len());
    for target in &plan.selected_targets {
      let name = target
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new(Status::InvalidArg, "selected module target has no name"))?;
      let build_option = plan
        .build_options
        .get(name)
        .or_else(|| plan.build_options.get("default"))
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));
      let paths = self
        .runtime
        .session
        .module_target_context_paths(root, name, &self.path_options)
        .map_err(Self::context_error)?;
      targets.push(serde_json::json!({
        "__arkdownFacade": "target",
        "targetName": name,
        "product": {
          "__arkdownFacade": "product",
          "productName": product.get("name").cloned().unwrap_or(Value::Null),
          "bundleType": product.get("bundleType").cloned().unwrap_or(Value::String("app".to_string())),
          "bundleName": bundle_name,
          "outputArtifactName": product.pointer("/output/artifactName").cloned().unwrap_or(Value::Null),
        },
        "buildOption": build_option,
        "paths": paths,
      }));
    }
    Ok(Value::Array(targets))
  }

  fn target_artifact_names(&self) -> Result<Value> {
    let plan = self
      .runtime
      .session
      .module_plan(self.module_root()?)
      .ok_or_else(|| Error::new(Status::InvalidArg, "runtime plan has no module target plan"))?;
    Ok(Value::Array(
      plan
        .selected_targets
        .iter()
        .filter_map(|target| {
          Some(Value::Array(vec![
            target.get("name")?.clone(),
            target.pointer("/output/artifactName")?.clone(),
          ]))
        })
        .collect(),
    ))
  }

  fn target_artifact_name(&self) -> Result<Value> {
    let target_name = self.argument.as_ref().and_then(Value::as_str);
    let plan = self
      .runtime
      .session
      .module_plan(self.module_root()?)
      .ok_or_else(|| Error::new(Status::InvalidArg, "runtime plan has no module target plan"))?;
    Ok(
      plan
        .selected_targets
        .iter()
        .find(|target| target.get("name").and_then(Value::as_str) == target_name)
        .and_then(|target| target.pointer("/output/artifactName"))
        .cloned()
        .unwrap_or(Value::Null),
    )
  }

  fn path_value(&self, path: PathBuf) -> Result<Value> {
    serde_json::to_value(path)
      .map_err(|error| Error::new(Status::GenericFailure, error.to_string()))
  }

  fn context_error(error: impl std::fmt::Display) -> Error {
    Error::new(Status::GenericFailure, error.to_string())
  }
}
