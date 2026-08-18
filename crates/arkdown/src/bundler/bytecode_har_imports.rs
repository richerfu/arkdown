//! Bytecode HAR import rewriting through pkgContextInfo records.

use std::path::Path;

use rustc_hash::FxHashSet;

use crate::plan::BundleRuntimePackageInfo;
use arkdown_core::ArkdownError;
use arkdown_path::{ArtifactPathSerializer, OhmUrl};

pub(crate) struct HvigorBytecodeHarImports<'a> {
    pub(crate) runtime_packages: &'a BundleRuntimePackageInfo,
    pub(crate) package_names: FxHashSet<&'a str>,
    pub(crate) use_normalized_ohm_url: bool,
}

impl<'a> HvigorBytecodeHarImports<'a> {
    pub(crate) fn new(
        runtime_packages: &'a BundleRuntimePackageInfo,
        package_names: &'a [String],
        use_normalized_ohm_url: bool,
    ) -> Self {
        Self {
            runtime_packages,
            package_names: package_names.iter().map(String::as_str).collect(),
            use_normalized_ohm_url,
        }
    }

    pub(crate) fn ohm_url_for_request(
        &self,
        request: &str,
    ) -> Result<Option<String>, ArkdownError> {
        if !self.use_normalized_ohm_url {
            return Ok(None);
        }
        if let Some(ohm_url) = self.normalized_so_ohm_url(request) {
            return Ok(Some(ohm_url));
        }
        if self.package_names.is_empty() {
            return Ok(None);
        }
        let Some(package_name) = self.alias_for_request(request) else {
            return Ok(None);
        };
        Ok(Some(self.normalized_ohm_url(package_name, request)?))
    }

    pub(crate) fn normalized_so_ohm_url(&self, request: &str) -> Option<String> {
        let context = self.runtime_packages.packages.get(request)?;
        if !context.is_so {
            return None;
        }
        Some(
            OhmUrl::normalized_so(
                context.module_name.as_str(),
                context.bundle_name.as_str(),
                Path::new(request)
                    .file_name()
                    .and_then(|file_name| file_name.to_str())
                    .unwrap_or(request),
                context.version.as_str(),
            )
            .into_string(),
        )
    }

    pub(crate) fn alias_for_request(&self, request: &str) -> Option<&'a str> {
        if let Some(package_name) = self.package_names.get(request).copied() {
            return Some(package_name);
        }
        self.package_names
            .iter()
            .copied()
            .filter(|package_name| {
                request
                    .strip_prefix(*package_name)
                    .is_some_and(|remaining| remaining.as_bytes().first() == Some(&b'/'))
            })
            .max_by_key(|package_name| package_name.len())
    }

    pub(crate) fn normalized_ohm_url(
        &self,
        package_name: &str,
        request: &str,
    ) -> Result<String, ArkdownError> {
        let context = self
            .runtime_packages
            .packages
            .get(package_name)
            .ok_or_else(|| {
                ArkdownError::Bundler(format!(
                    "pkgContextInfo is missing runtime package context for bytecode HAR '{}'",
                    package_name
                ))
            })?;
        let record = if request == package_name {
            if context.entry_path.is_empty() {
                return Err(ArkdownError::Bundler(format!(
                    "pkgContextInfo is missing bytecode HAR entryPath for '{}'",
                    package_name
                )));
            }
            self.remove_module_suffix(&format!("{package_name}/{}", context.entry_path))
        } else {
            request.replacen(package_name, &context.package_name, 1)
        };
        Ok(OhmUrl::normalized(
            context.is_so,
            context.module_name.as_str(),
            context.bundle_name.as_str(),
            &ArtifactPathSerializer::rel_to_slash(Path::new(&record)),
            context.version.as_str(),
        )
        .into_string())
    }

    pub(crate) fn remove_module_suffix(&self, value: &str) -> String {
        let value = ArtifactPathSerializer::rel_to_slash(Path::new(value));
        for suffix in [".d.ets", ".d.ts", ".ets", ".ts", ".mjs", ".cjs", ".js"] {
            if let Some(stripped) = value.strip_suffix(suffix) {
                return stripped.to_string();
            }
        }
        value
    }
}
