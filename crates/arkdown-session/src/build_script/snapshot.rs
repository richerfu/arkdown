//! `BuildScriptSnapshot` — session snapshot for one build script file.
//!
//! Each discovered `hvigorfile.ts` or `hvigorconfig.ts` produces one
//! `BuildScriptSnapshot` after OXC static analysis.  The snapshot records the
//! script path, its content fingerprint, and the declarative effects that
//! arkdown extracted from the AST.
//!
//! # Upstream correspondence
//!
//! Replaces `HvigorScriptSnapshot` + `HvigorTaskGraphSnapshot` from the old
//! `hvigor_script.rs` / `hvigor_task_graph.rs` modules.  The new model is
//! purely declarative: no runtime execution, no JS bridge.

use arkdown_path::AbsPath;

use crate::build_script::effects::BuildScriptEffect;

// ---------------------------------------------------------------------------
// BuildScriptKind
// ---------------------------------------------------------------------------

/// The kind of a build script file.
///
/// # Upstream correspondence
///
/// `hvigorfile.ts` — per-project or per-module build script.
/// `hvigorconfig.ts` — project/module configuration overlay script.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildScriptKind {
    /// `hvigorfile.ts` — task/plugin/hook registrations and build option
    /// overlays.
    HvigorFile,
    /// `hvigorconfig.ts` — project/module configuration script.
    HvigorConfig,
}

impl std::fmt::Display for BuildScriptKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            BuildScriptKind::HvigorFile => "hvigorfile.ts",
            BuildScriptKind::HvigorConfig => "hvigorconfig.ts",
        })
    }
}

// ---------------------------------------------------------------------------
// BuildScriptFingerprint
// ---------------------------------------------------------------------------

/// Content fingerprint for a build script file.
///
/// Used to determine whether the script has changed since the last build,
/// which would require re-running OXC analysis and invalidating downstream
/// cache entries.
///
/// # Upstream correspondence
///
/// Corresponds to the file hash computed for `hvigorfile.ts` in
/// `invalid-cache-plugin.js`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildScriptFingerprint {
    /// FNV-1a hex content hash of the script file.
    pub content_hash: String,
    /// File size in bytes.
    pub size: u64,
}

impl BuildScriptFingerprint {
    /// Compute a fingerprint from the raw file bytes.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            content_hash: format!("{:016x}", arkdown_core::fnv1a64(bytes)),
            size: bytes.len() as u64,
        }
    }

    /// Compute a fingerprint by reading from the given path.
    pub fn from_path(path: &AbsPath) -> std::io::Result<Self> {
        let bytes = std::fs::read(path.as_path())?;
        Ok(Self::from_bytes(&bytes))
    }
}

// ---------------------------------------------------------------------------
// BuildScriptSnapshot
// ---------------------------------------------------------------------------

/// Session snapshot of one `hvigorfile.ts` or `hvigorconfig.ts` file.
///
/// Built once during session startup by [`crate::build_script::BuildScriptParser`].
/// Contains the script path, its content fingerprint, and all declarative
/// effects that OXC static analysis was able to extract.
///
/// # Upstream correspondence
///
/// Replaces `HvigorScriptSnapshot` (discovery + fingerprints) and
/// `HvigorTaskGraphSnapshot` (task/hook graph) from the old hvigor compat
/// modules.
#[derive(Debug, Clone)]
pub struct BuildScriptSnapshot {
    /// Absolute path to the script file.
    pub script_path: AbsPath,
    /// The kind of script (hvigorfile.ts vs hvigorconfig.ts).
    pub kind: BuildScriptKind,
    /// Whether this is a module-level script (vs. project-level).
    pub is_module_level: bool,
    /// The module name this script belongs to (if module-level).
    pub module_name: Option<String>,
    /// Content fingerprint for cache invalidation.
    pub fingerprint: BuildScriptFingerprint,
    /// Declarative effects extracted by OXC static analysis.
    pub effects: Vec<BuildScriptEffect>,
}

impl BuildScriptSnapshot {
    /// Return all hook registration effects from this script.
    pub fn hook_registrations(
        &self,
    ) -> impl Iterator<Item = &crate::build_script::effects::HookRegistrationEffect> {
        self.effects.iter().filter_map(|e| {
            if let BuildScriptEffect::HookRegistration(h) = e {
                Some(h)
            } else {
                None
            }
        })
    }

    /// Return the `compilePluginFile` override path, if any.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `ArkCompile.getHvigorPluginFile`.
    pub fn compile_plugin_file(&self) -> Option<&AbsPath> {
        self.effects.iter().find_map(|e| {
            if let BuildScriptEffect::CompilePluginFile(p) = e {
                Some(p)
            } else {
                None
            }
        })
    }

    /// Return the statically evaluated `config.ohos.overrides` object.
    pub fn ohos_overrides(&self) -> Option<&serde_json::Value> {
        self.effects.iter().find_map(|effect| {
            if let BuildScriptEffect::OhosOverrides(value) = effect {
                Some(value)
            } else {
                None
            }
        })
    }
}
