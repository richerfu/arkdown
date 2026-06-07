//! [`InputFingerprintSet`] — content hashes for all startup inputs.
//!
//! Cache keys must be built from `InputFingerprintSet` or explicit content
//! hashes. Placeholder hashes are forbidden.

use crate::project::snapshot::ProjectSnapshot;

/// A single content fingerprint (SHA-256 of file content).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Fingerprint(pub [u8; 32]);

impl Fingerprint {
    /// Compute the fingerprint of the given bytes.
    pub fn of_bytes(data: &[u8]) -> Self {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&result);
        Self(bytes)
    }

    /// Compute the fingerprint of the given file.
    pub fn of_file(path: &std::path::Path) -> std::io::Result<Self> {
        let data = std::fs::read(path)?;
        Ok(Self::of_bytes(&data))
    }

    /// The zero fingerprint (no content).
    pub fn zero() -> Self {
        Self([0u8; 32])
    }

    /// Return `true` if this is the zero fingerprint.
    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }
}

/// The set of all startup input fingerprints.
///
/// Used to build cache keys and to determine which stages need to re-run
/// when inputs change.
#[derive(Debug, Clone, Default)]
pub struct InputFingerprintSet {
    /// Hash of the build configuration.
    pub config: Fingerprint,
    /// Hash of the dependency graph inputs (all `oh-package.json5` files).
    pub package_graph: Fingerprint,
    /// Hash of all profile JSON input files.
    pub profiles: Fingerprint,
    /// Hash of all resource input files.
    pub resources: Fingerprint,
    /// Hash of all source files.
    pub sources: Fingerprint,
    /// Hash of the selected SDK metadata.
    pub sdk: Fingerprint,
}

impl InputFingerprintSet {
    /// Build the fingerprint set from a project snapshot.
    ///
    /// Hashes are computed lazily; this function returns zeros for complex
    /// inputs until the full implementation is ready.
    pub fn build_from_project(project: &ProjectSnapshot) -> Self {
        // Hash build-profile.json5.
        let config = Fingerprint::of_file(&project.root.join("build-profile.json5"))
            .unwrap_or_else(|_| Fingerprint::zero());

        // Hash all oh-package.json5 files.
        let package_graph = compute_package_hash(project);

        Self {
            config,
            package_graph,
            ..Default::default()
        }
    }
}

fn compute_package_hash(project: &ProjectSnapshot) -> Fingerprint {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();

    // Hash root oh-package.json5 if present.
    let root_pkg = project.root.join("oh-package.json5");
    if let Ok(data) = std::fs::read(&root_pkg) {
        hasher.update(&data);
    }

    // Hash module oh-package.json5 files.
    let mut module_paths: Vec<_> = project
        .modules
        .values()
        .filter_map(|m| m.package_path.clone())
        .collect();
    module_paths.sort(); // Deterministic order.
    for path in module_paths {
        if let Ok(data) = std::fs::read(&path) {
            hasher.update(&data);
        }
    }

    let result = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&result);
    Fingerprint(bytes)
}
