//! Plugin-path resolution for the JavaScript build-script boundary.
//!
//! JavaScript executes plugin modules, while Rust owns deterministic path
//! normalization and the extension probing order observed in hvigor 6.24.3.

use std::path::{Path, PathBuf};

use arkdown_path::lexical_normalize;

const PLUGIN_EXTENSION_ORDER: [&str; 4] = ["js", "cjs", "mjs", "ts"];

pub struct BuildScriptPluginPathResolver {
    base_dir: PathBuf,
}

impl BuildScriptPluginPathResolver {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    pub fn resolve(&self, candidate: &str) -> Option<PathBuf> {
        if candidate.is_empty() {
            return None;
        }
        let candidate = Path::new(candidate);
        let resolved = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            self.base_dir.join(candidate)
        };
        let resolved = lexical_normalize(&resolved);
        if resolved.exists() {
            return Some(resolved);
        }
        for extension in PLUGIN_EXTENSION_ORDER {
            let mut suffixed = resolved.as_os_str().to_os_string();
            suffixed.push(".");
            suffixed.push(extension);
            let suffixed = PathBuf::from(suffixed);
            if suffixed.exists() {
                return Some(lexical_normalize(&suffixed));
            }
        }
        Some(resolved)
    }
}
