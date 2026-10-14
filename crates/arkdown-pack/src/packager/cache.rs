use std::path::Path;

use arkdown_cache::{ArtifactCacheStore, ArtifactInputHasher, CacheError};

use super::archive::{LoaderOutEtsAsset, PackAuxiliaryArtifacts};
use super::{PackInput, Packager};

impl Packager {
    /// Current BasePackHapTask declares pack.info, processed profile/resources,
    /// native/ETS assets, context metadata and source maps as content inputs.
    /// Hash the actual writer plan and generated inputs, including extra ETS
    /// assets; timestamps alone never establish a cache hit.
    pub(super) fn package_completion(
        &self,
        input: &PackInput,
    ) -> Option<(ArtifactCacheStore, String)> {
        let directory = self.cache_directory.as_deref()?;
        let result = (|| -> Result<_, CacheError> {
            let store = ArtifactCacheStore::for_output(directory, "package", &input.output_path)?;
            let mut hash = ArtifactInputHasher::new("hap-package-v1");
            hash.value(&serde_json::to_vec(&self.config)?);
            hash.value(&self.compress_level.to_le_bytes());
            hash.value(&serde_json::to_vec(input)?);
            let mut direct_paths =
                vec![input.module_json.as_path(), input.resources_index.as_path()];
            direct_paths.extend(
                input
                    .pack_info_module_json
                    .iter()
                    .chain(input.pack_info_path.iter())
                    .chain(input.source_maps_path.iter())
                    .chain(input.pkg_context_info_path.iter())
                    .chain(input.pkg_sdk_info_path.iter())
                    .chain(input.privacy_profile_path.iter())
                    .chain(input.syscap_path.iter())
                    .chain(input.native_libs.iter())
                    .chain(input.additional_abc_paths.iter())
                    .chain(input.integrated_hsps.iter().map(|entry| &entry.hsp_path))
                    .map(|path| path.as_path()),
            );
            hash.files(&direct_paths)?;
            if let Some(root) = input.resources_src_dir.as_deref() {
                Self::hash_package_tree(&mut hash, root)?;
            }
            if let Some(abc) = input.abc_path.as_deref() {
                hash.file(abc)?;
                if let Some(root) = abc.parent() {
                    Self::hash_package_ets(&mut hash, root, &input.output_path)?;
                }
            }
            if input
                .aot_mode
                .as_deref()
                .is_some_and(|mode| !mode.is_empty())
            {
                let auxiliary = PackAuxiliaryArtifacts::new(input);
                for root in auxiliary.an_dir().into_iter().chain(auxiliary.ap_dir()) {
                    Self::hash_package_tree(&mut hash, &root)?;
                }
            }
            Ok((store, hash.finish()))
        })();
        match result {
            Ok(completion) => Some(completion),
            Err(error) => {
                tracing::warn!(%error, "package input fingerprint failed; rebuilding");
                None
            }
        }
    }

    fn hash_package_tree(hash: &mut ArtifactInputHasher, root: &Path) -> Result<(), CacheError> {
        hash.value(&serde_json::to_vec(root)?);
        if !root.is_dir() {
            hash.value(b"missing-directory");
            return Ok(());
        }
        let mut paths = Vec::new();
        for entry in walkdir::WalkDir::new(root).follow_links(false) {
            let entry = entry.map_err(std::io::Error::other)?;
            if entry.file_type().is_file() {
                paths.push(entry.into_path());
            }
        }
        paths.sort();
        for path in paths {
            hash.file(&path)?;
        }
        Ok(())
    }

    fn hash_package_ets(
        hash: &mut ArtifactInputHasher,
        root: &Path,
        output: &Path,
    ) -> Result<(), CacheError> {
        let canonical = arkdown_path::AbsPath::canonicalize(output)
            .ok()
            .map(arkdown_path::AbsPath::into_path_buf);
        let mut paths = Vec::new();
        for entry in walkdir::WalkDir::new(root).follow_links(false) {
            let entry = entry.map_err(std::io::Error::other)?;
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let relative = path.strip_prefix(root).map_err(std::io::Error::other)?;
            if !LoaderOutEtsAsset::new(path, relative, output, canonical.as_deref())
                .is_generated_artifact()
            {
                paths.push(entry.into_path());
            }
        }
        paths.sort();
        for path in paths {
            hash.file(&path)?;
        }
        Ok(())
    }
}
