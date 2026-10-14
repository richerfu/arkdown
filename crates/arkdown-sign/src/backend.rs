use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use arkdown_core::SigningConfig;
use hapsigner::HapSigner;

use crate::config_material::SigningConfigMaterial;
use crate::error::SignError;
use crate::plan::SignPlan;

/// Sequential signing-stage runner backed by the published hapsigner crate.
#[derive(Debug, Default, Clone)]
pub struct SignBackendRunner {
    cache_directory: Option<PathBuf>,
}

impl SignBackendRunner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_cache_directory(mut self, directory: Option<PathBuf>) -> Self {
        self.cache_directory = directory;
        self
    }

    /// Execute an already resolved local-signing plan.
    pub fn run(&self, plan: &SignPlan) -> Result<(), SignError> {
        plan.validate()?;
        let material = plan.material().load()?;
        HapSigner::new(material, plan.options()).sign_file(plan.input(), plan.output())?;
        Ok(())
    }

    /// Sign using the selected build-profile entry, or preserve the unsigned
    /// archive when signing is not configured.
    pub fn sign_or_skip(
        &self,
        input: &Path,
        output: &Path,
        config: Option<&SigningConfig>,
    ) -> Result<(), SignError> {
        let Some(config) = config else {
            if input != output {
                if let Some(parent) = output.parent() {
                    fs_err::create_dir_all(parent)?;
                }
                fs_err::copy(input, output)?;
            }
            tracing::warn!(output = %output.display(), "no signing config; output is unsigned");
            return Ok(());
        };

        let plan = SigningConfigMaterial::new(config).plan(input, output)?;
        let completion = self.signing_completion(input, output, config);
        if let Some((store, key)) = &completion {
            match store.matches(key, &[output.to_path_buf()]) {
                Ok(true) => {
                    tracing::info!(output = %output.display(), "sign cache hit");
                    return Ok(());
                }
                Ok(false) => {}
                Err(error) => tracing::warn!(%error, "sign cache validation failed; signing again"),
            }
            if let Err(error) = store.invalidate() {
                tracing::warn!(%error, "sign cache invalidation failed");
            }
        }
        // SignHap.doTaskAction removes stale output only when the task runs.
        if output.is_file() && input != output {
            fs_err::remove_file(output)?;
        }
        self.run(&plan)?;
        if let Some((store, key)) = completion {
            if let Err(error) = store.commit(&key, &[output.to_path_buf()]) {
                tracing::warn!(%error, "sign completion cache commit failed");
            }
        }
        Ok(())
    }

    /// SignUtil declares algorithm/alias/password options plus certificate,
    /// profile, keystore and unsigned archive contents as signing inputs.
    fn signing_completion(
        &self,
        input: &Path,
        output: &Path,
        config: &SigningConfig,
    ) -> Option<(arkdown_cache::ArtifactCacheStore, String)> {
        let directory = self.cache_directory.as_deref()?;
        let result = (|| -> Result<_, arkdown_cache::CacheError> {
            let store = arkdown_cache::ArtifactCacheStore::for_output(directory, "sign", output)?;
            let mut hash = arkdown_cache::ArtifactInputHasher::new("hapsigner-sign-v1");
            hash.value(&serde_json::to_vec(config)?);
            for path in [
                input,
                config.cert_path.as_path(),
                config.profile_path.as_path(),
                config.store_file.as_path(),
            ] {
                hash.file(path)?;
            }
            // DevEcoPasswordResolver::resolve reads this material subtree.
            let material = SigningConfigMaterial::new(config)
                .material_dir()
                .join("material");
            hash.value(&serde_json::to_vec(&material)?);
            if material.is_dir() {
                let mut paths = Vec::new();
                for entry in walkdir::WalkDir::new(&material).follow_links(false) {
                    let entry = entry.map_err(std::io::Error::other)?;
                    if entry.file_type().is_file() {
                        paths.push(entry.into_path());
                    }
                }
                paths.sort();
                for path in paths {
                    hash.file(&path)?;
                }
            } else {
                hash.value(b"missing-password-material");
            }
            Ok((store, hash.finish()))
        })();
        match result {
            Ok(completion) => Some(completion),
            Err(error) => {
                tracing::warn!(%error, "sign input fingerprint failed; signing again");
                None
            }
        }
    }
}

/// HarmonyOS SDK directory-signing runner used by hvigor `PackageSignHar`.
///
/// HAP/HSP archives stay on the published `hapsigner` backend. HAR signing is
/// a different SDK protocol: `sdk-sign-tool-full.jar sign-har` mutates a task
/// directory containing `package/`, so the packager owns staging and this
/// runner owns only the typed external-tool invocation. This backend is an
/// explicit legacy item until hapsigner-rs exposes that HAR directory-signing
/// protocol; do not reproduce the protocol inside Arkdown.
#[derive(Debug, Clone)]
pub struct HarSignBackendRunner {
    java_executable: PathBuf,
}

impl Default for HarSignBackendRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl HarSignBackendRunner {
    pub fn new() -> Self {
        Self {
            java_executable: JavaExecutable::discover(),
        }
    }

    pub fn with_java_executable(java_executable: PathBuf) -> Self {
        Self { java_executable }
    }

    pub fn sign_directory(
        &self,
        signing_root: &Path,
        sdk_sign_tool: &Path,
        config: &SigningConfig,
    ) -> Result<(), SignError> {
        if !signing_root.is_absolute() || !signing_root.is_dir() {
            return Err(SignError::Config(format!(
                "HAR signing input must be an absolute directory: {}",
                signing_root.display()
            )));
        }
        if !sdk_sign_tool.is_absolute() || !sdk_sign_tool.is_file() {
            return Err(SignError::Config(format!(
                "HarmonyOS HAR signing tool is not a file: {}",
                sdk_sign_tool.display()
            )));
        }

        let material = SigningConfigMaterial::new(config).material()?;
        material.validate_paths()?;
        let store_password = material.store_password.resolve()?;
        let key_password = material.key_password.resolve()?;
        let command = HarSignCommandPlan::new(
            &self.java_executable,
            sdk_sign_tool,
            signing_root,
            &material,
            &store_password,
            &key_password,
        );
        let output = command.run().map_err(|source| SignError::IoPath {
            path: self.java_executable.clone(),
            source,
        })?;
        if !output.status.success() {
            return Err(SignError::ExternalTool {
                tool: sdk_sign_tool.to_path_buf(),
                status: output.status.to_string(),
                detail: self.sanitized_failure_detail(&output, [&store_password, &key_password]),
            });
        }
        Ok(())
    }

    fn sanitized_failure_detail<const N: usize>(
        &self,
        output: &Output,
        secrets: [&str; N],
    ) -> String {
        let bytes = if output.stderr.is_empty() {
            output.stdout.as_slice()
        } else {
            output.stderr.as_slice()
        };
        let mut detail = String::from_utf8_lossy(bytes).trim().to_string();
        for secret in secrets.into_iter().filter(|secret| !secret.is_empty()) {
            detail = detail.replace(secret, "[REDACTED]");
        }
        if detail.is_empty() {
            "no diagnostic output".to_string()
        } else {
            detail
        }
    }
}

struct JavaExecutable;

impl JavaExecutable {
    fn discover() -> PathBuf {
        let executable = OsString::from(format!("java{}", std::env::consts::EXE_SUFFIX));
        std::env::var_os("JAVA_HOME")
            .filter(|home| !home.is_empty())
            .map(|home| PathBuf::from(home).join("bin").join(&executable))
            .filter(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from(executable))
    }
}

struct HarSignCommandPlan {
    program: PathBuf,
    arguments: Vec<OsString>,
}

impl HarSignCommandPlan {
    fn new(
        java_executable: &Path,
        sdk_sign_tool: &Path,
        signing_root: &Path,
        material: &crate::plan::SigningMaterial,
        store_password: &str,
        key_password: &str,
    ) -> Self {
        Self {
            program: java_executable.to_path_buf(),
            arguments: vec![
                OsString::from("-jar"),
                sdk_sign_tool.as_os_str().to_owned(),
                OsString::from("sign-har"),
                OsString::from("-keystoreFile"),
                material.keystore_path.as_os_str().to_owned(),
                OsString::from("-keystorePwd"),
                OsString::from(store_password),
                OsString::from("-keyAlias"),
                OsString::from(&material.key_alias),
                OsString::from("-keyPwd"),
                OsString::from(key_password),
                OsString::from("-signAlg"),
                OsString::from(&material.sign_alg),
                OsString::from("-profileFile"),
                material.profile_path.as_os_str().to_owned(),
                OsString::from("-signCertFile"),
                material.cert_path.as_os_str().to_owned(),
                OsString::from("-inFile"),
                signing_root.as_os_str().to_owned(),
            ],
        }
    }

    fn run(&self) -> std::io::Result<Output> {
        Command::new(&self.program).args(&self.arguments).output()
    }
}

#[cfg(test)]
mod har_tests {
    use super::HarSignCommandPlan;
    use crate::plan::{SigningMaterial, SigningPassword};
    use std::ffi::{OsStr, OsString};
    use std::path::{Path, PathBuf};

    #[test]
    fn package_sign_har_command_matches_hvigor_har_sign_command_builder() {
        let material = SigningMaterial {
            keystore_path: PathBuf::from("/material/debug.p12"),
            key_alias: "debugKey".to_string(),
            store_password: SigningPassword::plaintext("unused"),
            key_password: SigningPassword::plaintext("unused"),
            profile_path: PathBuf::from("/material/profile.p7b"),
            cert_path: PathBuf::from("/material/cert.cer"),
            sign_alg: "SHA256withECDSA".to_string(),
            profile_signed: true,
        };
        let command = HarSignCommandPlan::new(
            Path::new("java"),
            Path::new("/sdk/toolchains/lib/sdk-sign-tool-full.jar"),
            Path::new("/build/cache/default@PackageSignHar"),
            &material,
            "store-secret",
            "key-secret",
        );
        let arguments = command
            .arguments
            .iter()
            .map(OsString::as_os_str)
            .collect::<Vec<_>>();
        assert_eq!(
            arguments,
            [
                OsStr::new("-jar"),
                OsStr::new("/sdk/toolchains/lib/sdk-sign-tool-full.jar"),
                OsStr::new("sign-har"),
                OsStr::new("-keystoreFile"),
                OsStr::new("/material/debug.p12"),
                OsStr::new("-keystorePwd"),
                OsStr::new("store-secret"),
                OsStr::new("-keyAlias"),
                OsStr::new("debugKey"),
                OsStr::new("-keyPwd"),
                OsStr::new("key-secret"),
                OsStr::new("-signAlg"),
                OsStr::new("SHA256withECDSA"),
                OsStr::new("-profileFile"),
                OsStr::new("/material/profile.p7b"),
                OsStr::new("-signCertFile"),
                OsStr::new("/material/cert.cer"),
                OsStr::new("-inFile"),
                OsStr::new("/build/cache/default@PackageSignHar"),
            ]
        );
    }
}
