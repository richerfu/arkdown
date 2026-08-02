use std::fmt::Write;
use std::path::{Path, PathBuf};

use arkdown_path::lexical_normalize;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BuildScriptProjectCacheIdentityError {
    #[error("failed to resolve the current directory: {0}")]
    CurrentDirectory(#[source] std::io::Error),
}

/// Latest-hvigor project cache identity derived from the normalized project
/// root. The startup/session layer owns cache layout and host path handling.
#[derive(Debug, Clone)]
pub struct BuildScriptProjectCacheIdentity {
    project_root: PathBuf,
}

impl BuildScriptProjectCacheIdentity {
    pub fn new(
        project_root: impl Into<PathBuf>,
    ) -> Result<Self, BuildScriptProjectCacheIdentityError> {
        let project_root = project_root.into();
        let project_root = if project_root.is_absolute() {
            project_root
        } else {
            std::env::current_dir()
                .map_err(BuildScriptProjectCacheIdentityError::CurrentDirectory)?
                .join(project_root)
        };
        Ok(Self {
            project_root: lexical_normalize(&project_root),
        })
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn hash(&self) -> String {
        let digest = Sha256::digest(self.project_root.to_string_lossy().as_bytes());
        let mut hash = String::with_capacity(32);
        for byte in &digest[..16] {
            let _ = write!(hash, "{byte:02x}");
        }
        hash
    }

    pub fn workspace(&self, hvigor_user_home: &Path) -> PathBuf {
        lexical_normalize(
            &hvigor_user_home
                .join("project_caches")
                .join(self.hash())
                .join("workspace"),
        )
    }
}
