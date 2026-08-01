//! ByteCode HAR `declarationEntry` collection.
//!
//! When arkdown builds a HAR target, it must write
//! `oh-package.json5#metadata.declarationEntry` — a list of OHM URLs that
//! describes the reachable module entry points for workers, runtimeOnly
//! sources/packages, ability, startup tasks, and insight intent entries.
//! Downstream HAP/HSP consumers read this list from `byteCodeHarInfo` in
//! their `loader.json`.
//!
//! # Upstream correspondence
//!
//! | Type/function | hvigor equivalent |
//! |---------------|-------------------|
//! | `ByteCodeHarDeclarationCollector` | `process-har-artifacts.ts::processDeclarationEntry` |
//! | `declarationEntry` field | `metadata.declarationEntry` in `oh-package.json5` |
//! | loader consumer | `generate-loader-json.ts#setByteCodeHarInfoAndOtherCompileFiles` |
//!
//! In hvigor's model, `declarationEntry` is populated by
//! `processDeclarationEntry()` which calls:
//! - `collectConfigOhmurl(workers, ...)` — worker ability OHM URLs
//! - `collectRuntimeOnlyOhmurl(runtimeOnly, ...)` — runtimeOnly sources/packages
//! - `collectByteCodeHarAbilityOhmurl(...)` — ability srcEntry OHM URLs
//! - `collectByteCodeHarStartupTasks(...)` — startup task OHM URLs
//! - `collectByteCodeHarInsightIntentSrcEntry(...)` — insight intent OHM URLs
//!
//! Arkdown reads those same typed sources and resolves every source through
//! the package's `pkgContextInfo`. It intentionally does not infer declaration
//! entries from loader `compileEntry`, whose set is broader.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use arkdown_core::{ArkdownConfig, HapManifest, ModuleConfig};
use arkdown_path::{ArtifactPathSerializer, OhmUrl};
use arkdown_profile::{PkgContextInfoInput, PkgContextInfoOpt};
use serde::Deserialize;

use super::layout::{ModuleBuildLayout, ModuleSourceLayout};

// ---------------------------------------------------------------------------
// ByteCodeHarDeclarationEntry
// ---------------------------------------------------------------------------

/// The `declarationEntry` list for a ByteCode HAR being built.
///
/// A sorted, deduplicated set of OHM URL strings representing all reachable
/// module entry points (workers, abilities, startup tasks, insight intents,
/// runtimeOnly sources/packages) in the HAR.
///
/// Written to `oh-package.json5#metadata.declarationEntry` in the HAR
/// archive so downstream consumers can find entry points without re-scanning
/// the HAR's source files.
#[derive(Debug, Clone, Default)]
pub struct ByteCodeHarDeclarationEntry {
  /// Sorted, deduplicated OHM URL entry points for this ByteCode HAR.
  pub ohm_urls: Vec<String>,
}

// ---------------------------------------------------------------------------
// ByteCodeHarDeclarationCollector
// ---------------------------------------------------------------------------

/// Collects `declarationEntry` for a HAR being built by arkdown.
///
/// Reads entry points from the same source profiles and build options used by
/// Hvigor and returns them as the HAR's `declarationEntry`.
///
/// # Usage
///
/// Call [`ByteCodeHarDeclarationCollector::collect`] after the profile stage
/// produces a [`LoaderJson`] for the HAR module. The package assembler writes
/// the result into its in-memory manifest; the module's source
/// `oh-package.json5` is never modified.
pub struct ByteCodeHarDeclarationCollector<'a> {
  config: &'a ArkdownConfig,
  module: &'a ModuleConfig,
  manifest: &'a HapManifest,
  package_input: &'a PkgContextInfoInput,
  package_contexts: &'a std::collections::BTreeMap<String, PkgContextInfoOpt>,
}

impl<'a> ByteCodeHarDeclarationCollector<'a> {
  pub fn new(
    config: &'a ArkdownConfig,
    module: &'a ModuleConfig,
    manifest: &'a HapManifest,
    package_input: &'a PkgContextInfoInput,
    package_contexts: &'a std::collections::BTreeMap<String, PkgContextInfoOpt>,
  ) -> Self {
    Self {
      config,
      module,
      manifest,
      package_input,
      package_contexts,
    }
  }

  /// Collect the exact inputs used by
  /// `process-har-artifacts.ts::processDeclarationEntry`.
  ///
  /// This deliberately does not consume loader `compileEntry`: pages, module
  /// mains, forms, startup configEntry, and dependency abilities are compiler
  /// entrances but are not ByteCode HAR declaration entrances.
  pub fn collect(&self) -> Result<ByteCodeHarDeclarationEntry> {
    let mut entries = BTreeSet::new();
    let current_context = self
      .package_contexts
      .get(&self.package_input.current.package_name)
      .with_context(|| {
        format!(
          "pkgContextInfo for ByteCode HAR package '{}' was not generated",
          self.package_input.current.package_name
        )
      })?;

    let current_metadata = self.read_package_declaration_config(&self.module.src_path);
    let configured_workers = self
      .config
      .product
      .build_option
      .source_option
      .as_ref()
      .map(|source| source.workers.clone())
      .filter(|workers| !workers.is_empty())
      .unwrap_or_else(|| {
        current_metadata
          .as_ref()
          .and_then(|metadata| metadata.workers.clone())
          .unwrap_or_default()
          .into_iter()
          .map(PathBuf::from)
          .collect()
      });
    for worker in configured_workers {
      self.insert_source_path(
        &mut entries,
        current_context,
        &self.module.src_path,
        &worker,
      )?;
    }

    let configured_runtime_sources = self
      .config
      .product
      .build_option
      .ark_options
      .as_ref()
      .and_then(|options| options.runtime_only.as_ref())
      .map(|runtime_only| runtime_only.sources.clone())
      .filter(|sources| !sources.is_empty())
      .unwrap_or_else(|| {
        current_metadata
          .as_ref()
          .and_then(|metadata| metadata.runtime_only.as_ref())
          .and_then(|runtime_only| runtime_only.sources.clone())
          .unwrap_or_default()
      });
    for source in configured_runtime_sources {
      self.insert_source(
        &mut entries,
        current_context,
        &self.module.src_path,
        &source,
      )?;
    }

    self.collect_current_ability_sources(current_context, &mut entries)?;
    self.collect_startup_tasks(current_context, &mut entries)?;
    self.collect_insight_intent_sources(current_context, &mut entries)?;

    if self.bundles_dependencies() {
      self.collect_bundled_dependency_sources(&mut entries)?;
    }

    Ok(ByteCodeHarDeclarationEntry {
      ohm_urls: entries.into_iter().collect(),
    })
  }

  fn bundles_dependencies(&self) -> bool {
    self
      .config
      .product
      .build_option
      .ark_options
      .as_ref()
      .is_some_and(|options| options.byte_code_har && options.bundled_dependencies)
  }

  fn collect_current_ability_sources(
    &self,
    context: &PkgContextInfoOpt,
    entries: &mut BTreeSet<String>,
  ) -> Result<()> {
    let src_main = ModuleSourceLayout::new(&self.module.src_path).src_main();
    for source in self
      .manifest
      .abilities
      .iter()
      .map(|ability| ability.src_entry.as_str())
      .chain(
        self
          .manifest
          .extension_abilities
          .iter()
          .map(|ability| ability.src_entry.as_str()),
      )
    {
      if source.ends_with(".so") {
        continue;
      }
      let path = src_main.join(arkdown_path::strip_relative_prefix(source));
      if path.is_file() {
        self.insert_source_path(entries, context, &self.module.src_path, &path)?;
      }
    }
    Ok(())
  }

  fn collect_startup_tasks(
    &self,
    context: &PkgContextInfoOpt,
    entries: &mut BTreeSet<String>,
  ) -> Result<()> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct StartupConfig {
      #[serde(default)]
      startup_tasks: Vec<StartupTask>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct StartupTask {
      ohmurl: Option<String>,
      src_entry: Option<String>,
      src_entry_absolute_path: Option<PathBuf>,
    }

    let path = ModuleBuildLayout::new(&self.module.src_path).startup_config_temp_json();
    let Some(config) = self.read_json5::<StartupConfig>(&path) else {
      return Ok(());
    };
    for task in config.startup_tasks {
      if let Some(ohmurl) = task.ohmurl.filter(|url| OhmUrl::parse(url.clone()).is_ok()) {
        entries.insert(ohmurl);
      } else if let Some(path) = task.src_entry_absolute_path {
        self.insert_source_path(entries, context, &self.module.src_path, &path)?;
      } else if let Some(source) = task.src_entry {
        self.insert_source(entries, context, &self.module.src_path, &source)?;
      }
    }
    Ok(())
  }

  fn collect_insight_intent_sources(
    &self,
    context: &PkgContextInfoOpt,
    entries: &mut BTreeSet<String>,
  ) -> Result<()> {
    let profile = ModuleSourceLayout::new(&self.module.src_path)
      .resources()
      .join("base/profile/insight_intent.json");
    let Some(profile) = self.read_json5::<InsightIntentProfile>(&profile) else {
      return Ok(());
    };
    for intent in profile.insight_intents_src_entry {
      if let Some(source) = intent.src_entry {
        let path = self
          .module
          .src_path
          .join("src")
          .join("main")
          .join(arkdown_path::strip_relative_prefix(&source));
        self.insert_source_path(entries, context, &self.module.src_path, &path)?;
      }
    }
    Ok(())
  }

  fn collect_bundled_dependency_sources(&self, entries: &mut BTreeSet<String>) -> Result<()> {
    for dependency in self
      .package_input
      .dependencies
      .iter()
      .filter(|dependency| dependency.kind == arkdown_profile::PkgContextDependencyKind::Har)
    {
      let Some(root) = dependency.package_root.as_deref() else {
        continue;
      };
      let Some(context) = self.package_contexts.get(&dependency.package_name) else {
        continue;
      };
      let package_config = self.read_package_declaration_config(root);
      let build_config = self.read_dependency_build_config(root);

      let workers = build_config
        .as_ref()
        .and_then(|config| config.workers.clone())
        .filter(|workers| !workers.is_empty())
        .or_else(|| {
          package_config
            .as_ref()
            .and_then(|config| config.workers.clone())
        })
        .unwrap_or_default();
      for worker in workers {
        self.insert_source(entries, context, root, &worker)?;
      }

      let runtime_only = build_config
        .as_ref()
        .and_then(|config| config.runtime_only.clone())
        .or_else(|| {
          package_config
            .as_ref()
            .and_then(|config| config.runtime_only.clone())
        })
        .unwrap_or_default();
      for source in runtime_only.sources.unwrap_or_default() {
        self.insert_source(entries, context, root, &source)?;
      }
      for package in runtime_only.packages.unwrap_or_default() {
        let Some(runtime_dependency) = self
          .package_input
          .dependencies
          .iter()
          .find(|candidate| candidate.alias == package || candidate.package_name == package)
        else {
          continue;
        };
        let Some(runtime_root) = runtime_dependency.package_root.as_deref() else {
          continue;
        };
        let Some(runtime_context) = self.package_contexts.get(&runtime_dependency.package_name)
        else {
          continue;
        };
        if let Some(main) = runtime_dependency.main.as_deref() {
          self.insert_source(entries, runtime_context, runtime_root, main)?;
        }
      }

      self.collect_dependency_router_sources(root, context, entries)?;
      self.collect_dependency_insight_sources(root, context, entries)?;
    }
    Ok(())
  }

  fn collect_dependency_router_sources(
    &self,
    root: &Path,
    context: &PkgContextInfoOpt,
    entries: &mut BTreeSet<String>,
  ) -> Result<()> {
    #[derive(Deserialize)]
    struct ModuleProfile {
      module: ModuleRouterMap,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ModuleRouterMap {
      router_map: Option<String>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RouterProfile {
      #[serde(default)]
      router_map: Vec<RouterEntry>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RouterEntry {
      page_source_file: Option<String>,
    }

    let module_profile = [
      ModuleSourceLayout::new(root).module_json5(),
      ModuleSourceLayout::new(root).src_main().join("module.json"),
    ]
    .into_iter()
    .find_map(|path| self.read_json5::<ModuleProfile>(&path));
    let Some(profile_name) = module_profile
      .and_then(|profile| profile.module.router_map)
      .and_then(|reference| reference.strip_prefix("$profile:").map(str::to_string))
    else {
      return Ok(());
    };
    let profile = ModuleSourceLayout::new(root)
      .resources()
      .join("base/profile")
      .join(profile_name)
      .with_extension("json");
    let Some(profile) = self.read_json5::<RouterProfile>(&profile) else {
      return Ok(());
    };
    for route in profile.router_map {
      if let Some(source) = route.page_source_file {
        self.insert_source(entries, context, root, &source)?;
      }
    }
    Ok(())
  }

  fn collect_dependency_insight_sources(
    &self,
    root: &Path,
    context: &PkgContextInfoOpt,
    entries: &mut BTreeSet<String>,
  ) -> Result<()> {
    let profile = ModuleSourceLayout::new(root)
      .resources()
      .join("base/profile/insight_intent.json");
    let Some(profile) = self.read_json5::<InsightIntentProfile>(&profile) else {
      return Ok(());
    };
    for intent in profile.insight_intents_src_entry {
      if let Some(source) = intent.src_entry {
        let path = root
          .join("src")
          .join("main")
          .join(arkdown_path::strip_relative_prefix(&source));
        self.insert_source_path(entries, context, root, &path)?;
      }
    }
    Ok(())
  }

  fn insert_source(
    &self,
    entries: &mut BTreeSet<String>,
    context: &PkgContextInfoOpt,
    package_root: &Path,
    source: &str,
  ) -> Result<()> {
    if source.trim().is_empty() {
      return Ok(());
    }
    if OhmUrl::parse(source.to_string()).is_ok() {
      entries.insert(source.to_string());
      return Ok(());
    }
    self.insert_source_path(entries, context, package_root, Path::new(source))
  }

  fn insert_source_path(
    &self,
    entries: &mut BTreeSet<String>,
    context: &PkgContextInfoOpt,
    package_root: &Path,
    source: &Path,
  ) -> Result<()> {
    let relative = if source.has_root() {
      source.strip_prefix(package_root).with_context(|| {
        format!(
          "ByteCode HAR declaration source '{}' is outside package root '{}'",
          source.display(),
          package_root.display()
        )
      })?
    } else {
      source
    };
    let relative = ArtifactPathSerializer::rel_to_slash(relative);
    let relative = Self::remove_source_suffix(&relative);
    let normalized_import = if relative.is_empty() {
      context.package_name.clone()
    } else {
      format!("{}/{}", context.package_name, relative)
    };
    entries.insert(
      OhmUrl::normalized_non_so(
        &context.module_name,
        &context.bundle_name,
        &normalized_import,
        &context.version,
      )
      .into_string(),
    );
    Ok(())
  }

  fn read_package_declaration_config(&self, root: &Path) -> Option<DeclarationConfig> {
    #[derive(Deserialize)]
    struct PackageJson {
      metadata: Option<DeclarationConfig>,
    }
    self
      .read_json5::<PackageJson>(&root.join("oh-package.json5"))
      .and_then(|package| package.metadata)
  }

  fn read_dependency_build_config(&self, root: &Path) -> Option<DeclarationConfig> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct BuildProfile {
      build_option: Option<BuildOption>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct BuildOption {
      source_option: Option<SourceOption>,
      ark_options: Option<ArkOptions>,
    }
    #[derive(Deserialize)]
    struct SourceOption {
      workers: Option<Vec<String>>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ArkOptions {
      runtime_only: Option<RuntimeOnlyConfig>,
    }

    let profile = self.read_json5::<BuildProfile>(&root.join("build-profile.json5"))?;
    let build_option = profile.build_option?;
    Some(DeclarationConfig {
      workers: build_option.source_option.and_then(|source| source.workers),
      runtime_only: build_option.ark_options.and_then(|ark| ark.runtime_only),
    })
  }

  fn read_json5<T: for<'de> Deserialize<'de>>(&self, path: &Path) -> Option<T> {
    let content = fs_err::read_to_string(path).ok()?;
    json5::from_str(&content).ok()
  }

  fn remove_source_suffix(path: &str) -> &str {
    for suffix in [
      ".d.ets", ".d.ts", ".d.mjs", ".d.cjs", ".d.js", ".ets", ".ts", ".mjs", ".cjs", ".js", ".so",
    ] {
      if let Some(path) = path.strip_suffix(suffix) {
        return path;
      }
    }
    path
  }
}

#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeclarationConfig {
  workers: Option<Vec<String>>,
  runtime_only: Option<RuntimeOnlyConfig>,
}

#[derive(Clone, Default, Deserialize)]
struct RuntimeOnlyConfig {
  sources: Option<Vec<String>>,
  packages: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InsightIntentProfile {
  #[serde(default)]
  insight_intents_src_entry: Vec<InsightIntentEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InsightIntentEntry {
  src_entry: Option<String>,
}
