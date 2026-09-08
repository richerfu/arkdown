//! Startup-owned ETS tables from ets_checker.ts and external_component_map.ts.

use std::{path::Path, sync::Arc};

use arkdown_ohos_schema::ets_options::EtsOptions;
use rustc_hash::FxHashSet;
use serde::Deserialize;

use super::snapshot::SdkSnapshotError;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoaderConfig {
    compiler_options: LoaderCompilerOptions,
}

#[derive(Deserialize)]
struct LoaderCompilerOptions {
    #[serde(default)]
    ets: EtsOptions,
}

pub(super) struct SdkEtsOptions;

impl SdkEtsOptions {
    pub(super) fn load(
        loader: &Path,
        external_roots: &[std::path::PathBuf],
    ) -> Result<Option<Arc<EtsOptions>>, SdkSnapshotError> {
        let path = loader.join("tsconfig.json");
        // SDK discovery also supports tool-only inventories. A checker caller
        // requires this configuration; absence is not replaced with defaults.
        if !path
            .try_exists()
            .map_err(|source| SdkSnapshotError::ReadComponentConfig {
                path: path.clone(),
                source,
            })?
        {
            return Ok(None);
        }
        let mut options = Self::read(&path)?;
        for root in external_roots {
            let components = root.join("build-tools/ets-loader/components");
            if !components
                .try_exists()
                .map_err(|source| SdkSnapshotError::ReadComponentConfig {
                    path: components.clone(),
                    source,
                })?
            {
                continue;
            }
            let external_path = components.join("externalconfig.json");
            let external = Self::read(&external_path)?;
            // concatenateEtsOptions uses JS Set for names, retaining the first
            // occurrence. Component records are distinct objects, so even
            // equal-valued records are retained in their original order.
            let names = options
                .components
                .as_mut()
                .ok_or_else(|| Self::missing_array(&path, "components"))?;
            names.extend(
                external
                    .components
                    .ok_or_else(|| Self::missing_array(&external_path, "components"))?,
            );
            let mut seen = FxHashSet::default();
            names.retain(|name| seen.insert(name.clone()));
            options
                .extend
                .components
                .as_mut()
                .ok_or_else(|| Self::missing_array(&path, "extend.components"))?
                .extend(
                    external
                        .extend
                        .components
                        .ok_or_else(|| Self::missing_array(&external_path, "extend.components"))?,
                );
        }
        Ok(Some(Arc::new(options)))
    }

    fn missing_array(path: &Path, field: &str) -> SdkSnapshotError {
        // concatenateEtsOptions spreads all four arrays. Missing arrays are
        // malformed configuration, not an instruction to merge empty tables.
        SdkSnapshotError::InvalidComponentConfig {
            path: path.to_path_buf(),
            message: format!(
                "compilerOptions.ets.{field} must be an array for external component merging"
            ),
        }
    }

    fn read(path: &Path) -> Result<EtsOptions, SdkSnapshotError> {
        let text = std::fs::read_to_string(path).map_err(|source| {
            SdkSnapshotError::ReadComponentConfig {
                path: path.to_path_buf(),
                source,
            }
        })?;
        let config: LoaderConfig =
            json5::from_str(&text).map_err(|error| SdkSnapshotError::InvalidComponentConfig {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
        Ok(config.compiler_options.ets)
    }
}
