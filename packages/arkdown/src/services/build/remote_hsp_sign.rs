//! hvigor `SignRemoteHsp` chain parity (`sign-project-remote-hsp` /
//! `sign-module-remote-hsp`).
//!
//! When signing is enabled, hvigor scans every `oh_modules/.hsp` repository
//! (project + module roots) for remote HSP packages that appear in the
//! dependency graphs, signs each one with the project signing config, and
//! writes the signed copies under `build/cache/<product>/remote_hsp/<dir>/`
//! plus the `remoteHspCache.json` manifest (`signRemoteHspCallBack`). The
//! app packaging itself keeps consuming the unsigned copies (the app archive
//! is signed as a whole by `SignPackagesFromApp`), so this pass is only
//! observable through the cache tree.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;

use arkdown_core::ArkdownConfig;
use arkdown_pack::{RemoteHspCollector, RemoteHspDependencyScope};
use arkdown_session::BuildSession;
use arkdown_sign::SignBackendRunner;

/// Repository owned by one of hvigor's two remote-HSP signing tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RemoteHspSignScope {
  Project,
  Module,
}

/// Signs all remote HSP dependencies of every module into the hvigor
/// `build/cache/<product>/remote_hsp/` layout.
pub(super) struct RemoteHspSigner {
  config: Arc<ArkdownConfig>,
  session: Arc<BuildSession>,
}

impl RemoteHspSigner {
  pub(super) fn new(config: Arc<ArkdownConfig>, session: Arc<BuildSession>) -> Self {
    Self { config, session }
  }

  /// hvigor `SignProjectRemoteHsp`/`SignModuleRemoteHsp` doTaskAction for one
  /// module target. The project task scans only the project repository and the
  /// module task scans only the module repository; their shared cache manifest
  /// is merged as each task completes.
  pub(super) fn run_for_module(
    &self,
    module_name: &str,
    scope: RemoteHspSignScope,
  ) -> anyhow::Result<Option<PathBuf>> {
    let Some(signing_config) = self.config.product.signing_config.as_ref() else {
      return Ok(None);
    };

    let scopes = self
      .config
      .modules
      .iter()
      .filter(|module| module.name == module_name)
      .filter_map(|module| {
        let context = self.session.module_context(&module.name)?;
        Some(RemoteHspDependencyScope::new(
          module.src_path.clone(),
          context
            .deps
            .nodes
            .values()
            .map(|node| node.package_name.clone()),
        ))
      })
      .collect::<Vec<_>>();
    if scopes.is_empty() {
      return Ok(None);
    }

    let collector = RemoteHspCollector::new(&self.config.project_root, &scopes);
    let candidates = match scope {
      RemoteHspSignScope::Project => collector.collect_project_candidates(),
      RemoteHspSignScope::Module => collector.collect_module_candidates(),
    }
    .map_err(anyhow::Error::from)?;
    if candidates.is_empty() {
      return Ok(None);
    }

    let cache_root = self
      .config
      .project_root
      .join("build")
      .join("cache")
      .join(&self.config.product.name)
      .join("remote_hsp");
    fs_err::create_dir_all(&cache_root)
      .with_context(|| format!("create remote HSP cache at {}", cache_root.display()))?;

    let cache_file = cache_root.join("remoteHspCache.json");
    let sign_backend = SignBackendRunner::new();
    let mut signed_remote_hsps = Self::existing_signed_remote_hsps(&cache_file)?;
    for candidate in candidates.values() {
      // `initAllSignRemoteHspList`: `<file>.hsp` → `<file>-signed.hsp`
      // under the hsp directory name; `submitSignWorker` mkdirs the output
      // directory.
      let signed_file_name = candidate.file_name.replace(".hsp", "-signed.hsp");
      let output = cache_root
        .join(&candidate.directory_name)
        .join(&signed_file_name);
      if let Some(parent) = output.parent() {
        fs_err::create_dir_all(parent)
          .with_context(|| format!("create remote HSP output dir at {}", parent.display()))?;
      }
      if output.is_file() {
        // `submitSignWorker` skips existing outputs; the cache file still
        // records the entry.
        signed_remote_hsps.insert(
          candidate.directory_name.clone(),
          signed_cache_entry(candidate, &output),
        );
        continue;
      }
      sign_backend
        .sign_or_skip(&candidate.path, &output, Some(signing_config))
        .map_err(|error| {
          anyhow::anyhow!(
            "SignRemoteHsp failed for {} ({}): {error}",
            candidate.file_name,
            candidate.path.display()
          )
        })?;
      signed_remote_hsps.insert(
        candidate.directory_name.clone(),
        signed_cache_entry(candidate, &output),
      );
    }

    // `signRemoteHspCallBack`: the manifest is only written once every
    // required remote HSP is signed. The `signingConfig` block mirrors the
    // merged `{name, material}` entry hvigor stores in
    // `GlobalProjectDataService`.
    fs_err::write(
      &cache_file,
      serde_json::to_string_pretty(&serde_json::json!({
        "signingConfig": hvigor_signing_config_shape(&self.config, signing_config),
        "signedRemoteHsps": signed_remote_hsps,
      }))
      .expect("serialize remote HSP cache"),
    )
    .with_context(|| format!("write {}", cache_file.display()))?;
    Ok(Some(cache_file))
  }

  fn existing_signed_remote_hsps(
    cache_file: &std::path::Path,
  ) -> anyhow::Result<BTreeMap<String, serde_json::Value>> {
    if !cache_file.is_file() {
      return Ok(BTreeMap::new());
    }
    let bytes =
      fs_err::read(cache_file).with_context(|| format!("read {}", cache_file.display()))?;
    let value: serde_json::Value =
      serde_json::from_slice(&bytes).with_context(|| format!("parse {}", cache_file.display()))?;
    Ok(
      value
        .get("signedRemoteHsps")
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flat_map(|entries| entries.iter())
        .map(|(name, entry)| (name.clone(), entry.clone()))
        .collect(),
    )
  }
}

/// hvigor `getSigningConfig` merged entry shape: the `signingConfigs` list
/// entry name plus its `material` block.
fn hvigor_signing_config_shape(
  config: &ArkdownConfig,
  signing_config: &arkdown_core::SigningConfig,
) -> serde_json::Value {
  // The product references the entry by name (`s.substring(s.lastIndexOf(".")
  // + 1)`); resolve it from the raw profile when available.
  let name = arkdown_config::build_profile::RawBuildProfile::load(&config.project_root)
    .ok()
    .and_then(|profile| {
      profile
        .app
        .products
        .iter()
        .find(|product| product.name == config.product.name)
        .and_then(|product| product.signing_config.as_deref())
        .map(|reference| {
          reference
            .rsplit_once('.')
            .map(|(_, entry)| entry)
            .unwrap_or(reference)
        })
        .map(str::to_string)
    });
  serde_json::json!({
    "name": name,
    "material": {
      "certpath": signing_config.cert_path,
      "keyAlias": signing_config.key_alias,
      "keyPassword": signing_config.key_password,
      "profile": signing_config.profile_path,
      "signAlg": signing_config.sign_alg,
      "storeFile": signing_config.store_file,
      "storePassword": signing_config.store_password,
    },
  })
}

/// The per-HSP entry hvigor's `GlobalProjectDataService.setNeedSignedRemoteHspMap`
/// records in `remoteHspCache.json`.
fn signed_cache_entry(
  candidate: &arkdown_pack::RemoteHspCandidate,
  signed_path: &std::path::Path,
) -> serde_json::Value {
  serde_json::json!({
    "hspFileName": candidate.file_name,
    "hspDirName": candidate.directory_name,
    "version": candidate.version.to_string(),
    "hspPath": candidate.path,
    "signedHspPath": signed_path,
    "isSigned": true,
  })
}
