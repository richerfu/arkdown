//! hvigor `BuildInfo` task parity.
//!
//! Prints the build-profile projection hvigor's `build-info.ts` emits:
//! `product`/`module`/`buildMode` name lists plus a `buildInfo` tree of
//! `product → module → target → buildMode` entries. With
//! `-p buildOption=` hvigor fills each entry with the resolved effective
//! build option (`buildOptionManager.getTargetBuildOptionWithProductBuildMode`);
//! without it every entry is `{}`. `-p json=` selects JSON output, otherwise
//! hvigor prints a formatted tree under "All build information from hvigor
//! node '<node>'".
//!
//! ## Usage
//!
//! ```text
//! arkdown build-info --project .                      # formatted tree
//! arkdown build-info --project . --json               # JSON (`-p json=`)
//! arkdown build-info --project . --json --resolve     # JSON + resolved options (`-p buildOption=`)
//! arkdown build-info --project . --module entry       # module projection (`-p module=entry`)
//! ```
//!
//! The `--json` output can be diffed against `hvigorw build-info -p json=`
//! for oracle parity.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use arkdown_config::build_profile::{
  BuildOptionProfileInventory, EffectiveModuleBuildOptionResolver, RawBuildProfile,
};

/// hvigor `BuildInfo` task arguments.
#[derive(Args, Debug)]
pub struct BuildInfoArgs {
  /// Project root (default: current directory)
  #[arg(long)]
  pub project: Option<PathBuf>,

  /// Module projection (`-p module=<name>@<product>` — only the module part
  /// is used, matching `module.split("@")[0]`).
  #[arg(long)]
  pub module: Option<String>,

  /// JSON output (`-p json=`). Without this flag hvigor prints the
  /// indented "All build information" tree.
  #[arg(long)]
  pub json: bool,

  /// Resolve effective build options for every entry (`-p buildOption=`).
  /// Without this flag every buildInfo entry is `{}`.
  #[arg(long)]
  pub resolve: bool,
}

/// Same-process request for Hvigor's built-in `buildInfo` task.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorBuildInfoRequest {
  project_root: PathBuf,
  node_name: String,
  project_task: bool,
  #[serde(default)]
  module: Option<String>,
  #[serde(default)]
  json: bool,
  #[serde(default)]
  resolve: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HvigorBuildInfoResponse {
  output: Vec<String>,
}

/// Owns the file-backed build-profile projection used by the runtime task.
pub struct HvigorBuildInfoRunner {
  request: HvigorBuildInfoRequest,
}

impl HvigorBuildInfoRunner {
  pub fn from_value(request_value: Value) -> anyhow::Result<Self> {
    Ok(Self {
      request: serde_json::from_value(request_value)?,
    })
  }

  pub fn response_value(&self) -> anyhow::Result<Value> {
    serde_json::to_value(HvigorBuildInfoResponse {
      output: self.output()?,
    })
    .map_err(Into::into)
  }

  fn output(&self) -> anyhow::Result<Vec<String>> {
    // `BuildInfo.registryAction`: when `-p module=...` is present the project
    // task returns immediately and the selected module task owns the output.
    if self.request.project_task && self.request.module.is_some() {
      return Ok(Vec::new());
    }

    let info = if !self.request.project_task && self.request.module.is_none() {
      // A module-node buildInfo without `-p module` parses the module profile
      // through getProjectBuildInfo. Module profiles have no app/modules
      // inventory, so Hvigor emits this empty project-shaped projection.
      json!({
        "product": [],
        "module": [],
        "buildMode": [],
        "buildInfo": {},
      })
    } else {
      build_info_value(
        &self.request.project_root,
        self.request.module.as_deref(),
        self.request.resolve,
      )?
    };
    let output = if self.request.json {
      serde_json::to_string_pretty(&info)?
    } else {
      formatted_text(&self.request.node_name, &info)
    };
    Ok(vec![output])
  }
}

pub fn run(args: BuildInfoArgs) -> anyhow::Result<()> {
  let project_root = args
    .project
    .unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));
  let info = build_info_value(&project_root, args.module.as_deref(), args.resolve)?;

  if args.json {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &info)?;
    writeln!(stdout)?;
  } else {
    print_formatted(&project_root, args.module.as_deref(), &info)?;
  }
  Ok(())
}

/// Build the `BuildInfo` projection for a project root (hvigor
/// `BuildInfo::registryAction`): module projection when `module_param` is
/// present, project projection otherwise.
pub fn build_info_value(
  project_root: &Path,
  module_param: Option<&str>,
  resolve: bool,
) -> anyhow::Result<Value> {
  let profile = RawBuildProfile::load(project_root)
    .map_err(|error| anyhow::anyhow!("build-info failed: {error}"))?;

  // hvigor `BuildInfo` resolves build options per product/module/target/build
  // mode; reuse the same inventory and effective-option resolver as the
  // native build pipeline.
  let config = arkdown_config::load_config(
    project_root,
    arkdown_core::BuildMode::Debug,
    arkdown_core::OutputTarget::Hap,
    None,
    None,
  )?;
  let inventory =
    BuildOptionProfileInventory::load_with_runtime_overrides(project_root, &config.modules, None)
      .map_err(|error| anyhow::anyhow!("build-info failed: {error}"))?;

  let build_modes = profile
    .app
    .build_mode_set
    .iter()
    .map(|mode| mode.name.clone())
    .collect::<Vec<_>>();

  if let Some(module_param) = module_param {
    let module_name = module_param.split('@').next().unwrap_or(module_param);
    let module_build_info = module_build_info(
      &inventory,
      &profile,
      project_root,
      module_name,
      &build_modes,
      resolve,
    )?;
    Ok(json!({
      "buildMode": build_modes,
      "buildInfo": module_build_info,
    }))
  } else {
    let products = profile
      .app
      .products
      .iter()
      .map(|product| product.name.clone())
      .collect::<Vec<_>>();
    let modules = profile
      .modules
      .iter()
      .map(|module| module.name.clone())
      .collect::<Vec<_>>();
    let project_build_info = project_build_info(&inventory, &profile, &build_modes, resolve)?;
    Ok(json!({
      "product": products,
      "module": modules,
      "buildMode": build_modes,
      "buildInfo": project_build_info,
    }))
  }
}

/// hvigor `getModuleBuildInfo`: `{ buildMode: [...], buildInfo: {<target>:
/// {<buildMode>: option|{}}} }` for one module.
fn module_build_info(
  inventory: &BuildOptionProfileInventory,
  profile: &RawBuildProfile,
  project_root: &Path,
  module_name: &str,
  build_modes: &[String],
  resolve: bool,
) -> Result<Value, anyhow::Error> {
  let Some(raw_module) = profile.modules.iter().find(|m| m.name == module_name) else {
    return Ok(Value::Object(Map::new()));
  };
  // hvigor's module node enumerates the targets from the module's own
  // `build-profile.json5` (project-level `modules[].targets` only carries
  // the subset declared at project scope).
  let module_root = PathBuf::from(&raw_module.src_path);
  let module_profile_path = if module_root.is_absolute() {
    module_root.join("build-profile.json5")
  } else {
    project_root.join(&module_root).join("build-profile.json5")
  };
  let module_targets = std::fs::read_to_string(&module_profile_path)
    .ok()
    .and_then(|content| json5::from_str::<Value>(&content).ok())
    .and_then(|value| {
      value
        .get("targets")
        .and_then(serde_json::Value::as_array)
        .cloned()
    })
    .unwrap_or_else(|| {
      raw_module
        .targets
        .iter()
        .map(|target| json!({ "name": target.name }))
        .collect()
    });
  let mut targets = Map::new();
  for target in &module_targets {
    let target_name = target
      .get("name")
      .and_then(serde_json::Value::as_str)
      .unwrap_or("default")
      .to_string();
    let mut modes = Map::new();
    for build_mode in build_modes {
      // `getModuleBuildInfo` calls without a product — the manager defaults
      // to `"default"`.
      modes.insert(
        build_mode.clone(),
        build_option_entry(
          inventory,
          module_name,
          &target_name,
          build_mode,
          "default",
          resolve,
        ),
      );
    }
    targets.insert(target_name, Value::Object(modes));
  }
  Ok(Value::Object(targets))
}

/// hvigor `getProductBuildOption`: `{<product>: {<module>: {<target>:
/// {<buildMode>: option|{}}}}}` — modules are entered under each
/// `applyToProducts` entry of their targets.
fn project_build_info(
  inventory: &BuildOptionProfileInventory,
  profile: &RawBuildProfile,
  build_modes: &[String],
  resolve: bool,
) -> Result<Value, anyhow::Error> {
  let mut products = Map::new();
  for product in &profile.app.products {
    products.insert(product.name.clone(), Value::Object(Map::new()));
  }
  for raw_module in &profile.modules {
    for target in &raw_module.targets {
      let applied_products = if target.apply_to_products.is_empty() {
        vec!["default".to_string()]
      } else {
        target.apply_to_products.clone()
      };
      for product in applied_products {
        let product_entry = products
          .entry(product.clone())
          .or_insert_with(|| Value::Object(Map::new()))
          .as_object_mut()
          .ok_or_else(|| anyhow::anyhow!("buildInfo product entry is not an object"))?;
        let module_entry = product_entry
          .entry(raw_module.name.clone())
          .or_insert_with(|| Value::Object(Map::new()))
          .as_object_mut()
          .ok_or_else(|| anyhow::anyhow!("buildInfo module entry is not an object"))?;
        let mut modes = Map::new();
        for build_mode in build_modes {
          modes.insert(
            build_mode.clone(),
            build_option_entry(
              inventory,
              raw_module.name.as_str(),
              target.name.as_str(),
              build_mode,
              product.as_str(),
              resolve,
            ),
          );
        }
        module_entry.insert(target.name.clone(), Value::Object(modes));
      }
    }
  }
  Ok(Value::Object(products))
}

/// `buildOptionManager.getTargetBuildOptionWithProductBuildMode` — resolved
/// effective option when `-p buildOption=` was passed, `{}` otherwise.
fn build_option_entry(
  inventory: &BuildOptionProfileInventory,
  module_name: &str,
  target_name: &str,
  build_mode: &str,
  product: &str,
  resolve: bool,
) -> Value {
  if !resolve {
    return Value::Object(Map::new());
  }
  match EffectiveModuleBuildOptionResolver::new(
    inventory,
    module_name,
    product,
    build_mode,
    target_name,
  )
  .resolve_display_value()
  {
    Ok(value) => value,
    Err(error) => json!({ "_error": error.to_string() }),
  }
}

/// hvigor's formatted output: an indented key/value tree under the
/// "All build information from hvigor node '<node>'" banner. The node is
/// `:<module>` for module projections and `:` for the project.
fn print_formatted(
  project_root: &std::path::Path,
  module_param: Option<&str>,
  info: &Value,
) -> Result<(), anyhow::Error> {
  let node = module_param
    .map(|param| param.split('@').next().unwrap_or(param).to_string())
    .unwrap_or_else(|| ":".to_string());
  let output = formatted_text(&node, info);
  let mut stdout = std::io::stdout().lock();
  writeln!(stdout, "{output}")?;
  let _ = project_root;
  Ok(())
}

fn formatted_text(node: &str, info: &Value) -> String {
  let mut lines = vec![
    "--------------------------------------------------------------------------".to_string(),
    format!("All build information from hvigor node '{node}'"),
    "--------------------------------------------------------------------------".to_string(),
  ];
  append_formatted_recursively(info, &mut lines, 0);
  lines.join(if cfg!(windows) { "\r\n" } else { "\n" })
}

/// `createFormatInfoRecursively`: objects/arrays recurse with depth prefix,
/// strings/booleans print at their depth.
fn append_formatted_recursively(value: &Value, lines: &mut Vec<String>, depth: usize) {
  match value {
    Value::Object(entries) => {
      for (key, entry) in entries {
        lines.push(format!("{}{}", prefix(depth), key));
        append_formatted_recursively(entry, lines, depth + 1);
      }
    }
    Value::Array(entries) => {
      for entry in entries {
        append_formatted_recursively(entry, lines, depth);
      }
    }
    other => lines.push(format!("{}{}", prefix(depth), scalar_text(other))),
  }
}

fn scalar_text(value: &Value) -> String {
  match value {
    Value::String(text) => text.clone(),
    Value::Bool(flag) => flag.to_string(),
    Value::Null => "null".to_string(),
    other => other.to_string(),
  }
}

fn prefix(depth: usize) -> String {
  "    ".repeat(depth)
}
