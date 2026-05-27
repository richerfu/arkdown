use std::path::{Path, PathBuf};
use std::process::Command;
use arkdown_core::{BuildMode, ObfuscationConfig};

#[derive(Debug, thiserror::Error)]
pub enum ObfuscationError {
    #[error("Obfuscation tool not found at '{0}'. Check SDK installation.")]
    ToolNotFound(PathBuf),
    #[error("Obfuscation failed (exit {code}): {stderr}")]
    ToolFailed { code: i32, stderr: String },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Rule file not found: {0}")]
    RuleFileNotFound(PathBuf),
}

/// Configuration for the obfuscation step.
#[derive(Debug, Clone)]
pub struct ObfuscationOptions {
    /// Path to the SDK's obfuscation tool binary.
    pub tool_path: PathBuf,
    /// Rule files (--rule flags)
    pub rule_files: Vec<PathBuf>,
    /// Cache directory for obfuscation outputs
    pub cache_dir: PathBuf,
    /// Export rule path (consumer rules for HAR publishing)
    pub export_rule_path: Option<PathBuf>,
}

pub struct Obfuscator {
    options: ObfuscationOptions,
}

impl Obfuscator {
    pub fn new(options: ObfuscationOptions) -> Self {
        Obfuscator { options }
    }

    /// Obfuscate a JS file in place (or to a different output path).
    /// Returns the path to the obfuscated output.
    pub fn obfuscate_file(
        &self,
        input: &Path,
        output: &Path,
    ) -> Result<PathBuf, ObfuscationError> {
        if !self.options.tool_path.exists() {
            return Err(ObfuscationError::ToolNotFound(self.options.tool_path.clone()));
        }

        // Validate rule files exist
        for rule in &self.options.rule_files {
            if !rule.exists() {
                return Err(ObfuscationError::RuleFileNotFound(rule.clone()));
            }
        }

        let mut cmd = Command::new(&self.options.tool_path);
        cmd.arg("--input").arg(input);
        cmd.arg("--output").arg(output);
        for rule in &self.options.rule_files {
            cmd.arg("--rule").arg(rule);
        }
        if let Some(cache) = self.options.cache_dir.parent() {
            cmd.arg("--cache-dir").arg(cache);
        }

        let result = cmd.output().map_err(ObfuscationError::Io)?;
        if !result.status.success() {
            return Err(ObfuscationError::ToolFailed {
                code: result.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&result.stderr).to_string(),
            });
        }
        Ok(output.to_path_buf())
    }

    /// Obfuscate all .js files in a directory.
    /// In-place: overwrites each .js file with its obfuscated version.
    pub fn obfuscate_dir(&self, dir: &Path) -> Result<Vec<PathBuf>, ObfuscationError> {
        let mut outputs = Vec::new();
        for entry in walkdir::WalkDir::new(dir)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("js"))
        {
            let path = entry.path();
            let tmp = path.with_extension("js.obf_tmp");
            self.obfuscate_file(path, &tmp)?;
            std::fs::rename(&tmp, path)?;
            outputs.push(path.to_path_buf());
        }
        Ok(outputs)
    }
}

/// Compute a hash of all rule files for cache invalidation.
pub fn compute_rule_hash(rule_files: &[PathBuf]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for rule in rule_files {
        if let Ok(content) = std::fs::read(rule) {
            for b in &content {
                h ^= *b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
        }
    }
    h
}

/// Decide whether obfuscation should run for this build.
pub fn should_obfuscate(
    build_mode: &BuildMode,
    config: Option<&ObfuscationConfig>,
) -> bool {
    if !matches!(build_mode, BuildMode::Release) {
        return false;
    }
    config.map(|c| c.enable_obfuscation).unwrap_or(false)
}

/// Find the obfuscation tool from the SDK path.
pub fn find_obfuscation_tool(sdk_home: Option<&str>) -> Option<PathBuf> {
    let sdk = sdk_home
        .map(PathBuf::from)
        .or_else(|| std::env::var("OHOS_SDK_HOME").ok().map(PathBuf::from))?;
    let tool = sdk.join("default/openharmony/ets/build-tools/ark-tools/obfuscate");
    if tool.exists() {
        Some(tool)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_not_obfuscate_debug() {
        assert!(!should_obfuscate(&BuildMode::Debug, None));
    }

    #[test]
    fn test_should_not_obfuscate_release_disabled() {
        let cfg = ObfuscationConfig {
            enable_obfuscation: false,
            rules_file: None,
        };
        assert!(!should_obfuscate(&BuildMode::Release, Some(&cfg)));
    }

    #[test]
    fn test_should_obfuscate_release_enabled() {
        let cfg = ObfuscationConfig {
            enable_obfuscation: true,
            rules_file: None,
        };
        assert!(should_obfuscate(&BuildMode::Release, Some(&cfg)));
    }

    #[test]
    fn test_compute_rule_hash_deterministic() {
        let tmp = tempfile::tempdir().unwrap();
        let rule = tmp.path().join("rules.txt");
        std::fs::write(&rule, b"keep class *").unwrap();
        let h1 = compute_rule_hash(&[rule.clone()]);
        let h2 = compute_rule_hash(&[rule]);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_tool_not_found_error() {
        let opts = ObfuscationOptions {
            tool_path: PathBuf::from("/nonexistent/obfuscate-tool"),
            rule_files: vec![],
            cache_dir: PathBuf::from("/tmp"),
            export_rule_path: None,
        };
        let obfuscator = Obfuscator::new(opts);
        let result = obfuscator.obfuscate_file(Path::new("/some.js"), Path::new("/out.js"));
        assert!(matches!(result, Err(ObfuscationError::ToolNotFound(_))));
    }
}
