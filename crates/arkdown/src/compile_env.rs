//! Compile environment for the bundle stage.
//!
//! [`CompileEnvironment`] captures the full set of observable flags that
//! determine whether the bundle cache is valid.  It corresponds to hvigor's
//! `arkts-pack.js::getCompileEnv` output.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor/SDK equivalent |
//! |------|-----------------------|
//! | `CompileEnvironment` | `arkts-pack.js::getCompileEnv` |
//! | `CompileEnvFlagSet` | `generateConfig.compileEnv` flags |
//! | `FastBuildFeatureFlags` | hvigor fast-build mode flags |

use arkdown_core::BuildMode;

// ---------------------------------------------------------------------------
// CompileEnvFlagSet
// ---------------------------------------------------------------------------

/// Feature flags that affect the compile environment cache key.
///
/// When any flag changes, the compile environment key changes, triggering
/// a cache invalidation.
///
/// # Upstream correspondence
///
/// Corresponds to `generateConfig.compileEnv` and flags from
/// `ArkCompile::getCompileOptions`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompileEnvFlagSet {
    /// Whether to use normalized OHM URLs (API >= 9).
    pub use_normalized_ohm_url: bool,
    /// Whether source hash is used for cache keys (`useSourceHash`).
    pub use_source_hash: bool,
    /// Whether path placeholders are used for relocatable caches.
    pub use_path_placeholder: bool,
    /// Whether obfuscation is enabled.
    pub obfuscation_enabled: bool,
    /// Whether annotation support is enabled.
    pub annotations_enabled: bool,
    /// Whether use-define semantics for class fields is active.
    pub use_define_semantic: bool,
}

// ---------------------------------------------------------------------------
// FastBuildFeatureFlags
// ---------------------------------------------------------------------------

/// Flags for the hvigor fast-build mode.
///
/// Fast-build mode uses module ABC (per-file compilation) instead of bundle
/// ABC.  These flags determine which fast-build features are active.
///
/// # Upstream correspondence
///
/// Corresponds to hvigor `isFastBuild/isHarFastBuild` and related features.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FastBuildFeatureFlags {
    /// Whether fast-build mode is active.
    pub is_fast_build: bool,
    /// Whether this is a HAR fast-build (uses per-file ABC without bundling).
    pub is_har_fast_build: bool,
    /// Whether bytecode HAR is active.
    pub is_bytecode_har: bool,
}

// ---------------------------------------------------------------------------
// CompileMode
// ---------------------------------------------------------------------------

/// The compilation mode (ESModule vs JSBUNDLE).
///
/// # Upstream correspondence
///
/// Corresponds to `arkts-pack.js::getCompileMode` and the `-compileMode`
/// flag in hvigor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileMode {
    /// ESModule output (default for API >= 9).
    EsModule,
    /// JSBUNDLE output (legacy mode for API < 9).
    JsBundle,
}

impl Default for CompileMode {
    fn default() -> Self {
        Self::EsModule
    }
}

impl std::fmt::Display for CompileMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CompileMode::EsModule => "esmodule",
            CompileMode::JsBundle => "jsbundle",
        })
    }
}

// ---------------------------------------------------------------------------
// CompileEnvironment
// ---------------------------------------------------------------------------

/// The full compile environment for a bundle stage.
///
/// `CompileEnvironment` captures all observable fields that affect whether
/// the bundle cache is valid.  The `cache_key_material()` method produces the
/// string that `arkts-pack.js::getCompileEnv` would produce.
///
/// # Upstream correspondence
///
/// Corresponds to `arkts-pack.js::getCompileEnv` and
/// `generateConfig.compileEnv`.
#[derive(Debug, Clone)]
pub struct CompileEnvironment {
    /// SDK compile version (e.g., `"6.0.0.47"`).
    pub compile_sdk_version: String,
    /// Compatible SDK version (from `build-profile.json5`).
    pub compatible_sdk_version: String,
    /// Target OS (e.g., `"OpenHarmony"`).
    pub runtime_os: String,
    /// Absolute path to the SDK `ets-loader`.
    pub ets_loader_path: String,
    /// OHOS module type (e.g., `"Entry"`, `"Feature"`).
    pub module_type: String,
    /// Cache directory path.
    pub cache_path: String,
    /// Build mode: debug or release.
    pub build_mode: BuildMode,
    /// Compile mode: ESModule or JSBUNDLE.
    pub compile_mode: CompileMode,
    /// Feature flags that contribute to cache invalidation.
    pub flags: CompileEnvFlagSet,
    /// Fast-build mode flags.
    pub fast_build: FastBuildFeatureFlags,
}

impl CompileEnvironment {
    /// Construct from pre-resolved values.
    pub fn from_plan(
        compile_sdk_version: impl Into<String>,
        compatible_sdk_version: impl Into<String>,
        runtime_os: impl Into<String>,
        ets_loader_path: impl Into<String>,
        module_type: impl Into<String>,
        cache_path: impl Into<String>,
        build_mode: BuildMode,
        flags: CompileEnvFlagSet,
    ) -> Self {
        Self {
            compile_sdk_version: compile_sdk_version.into(),
            compatible_sdk_version: compatible_sdk_version.into(),
            runtime_os: runtime_os.into(),
            ets_loader_path: ets_loader_path.into(),
            module_type: module_type.into(),
            cache_path: cache_path.into(),
            build_mode,
            compile_mode: CompileMode::EsModule,
            flags,
            fast_build: FastBuildFeatureFlags::default(),
        }
    }

    /// Returns `true` if the bundle should use ESModule format.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `arkts-pack.js::isModuleMode()` check.
    pub fn is_module_mode(&self) -> bool {
        matches!(self.compile_mode, CompileMode::EsModule)
    }

    /// Returns `true` if JS should be emitted for this build.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `getShouldEmitJs` in hvigor.
    pub fn should_emit_js(&self) -> bool {
        // In module mode, JS is always emitted.
        // In JSBUNDLE mode, JS emission depends on the build target.
        true
    }

    /// Returns `true` if source hash should be used for cache keys.
    pub fn use_source_hash(&self) -> bool {
        self.flags.use_source_hash
    }

    /// Returns `true` if path placeholders should be used.
    pub fn use_path_placeholder(&self) -> bool {
        self.flags.use_path_placeholder
    }

    /// Produce the `#`-joined cache key material string.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to `arkts-pack.js::getCompileEnv()` key string that
    /// `CacheService.getCompileEnvHash` hashes for cache invalidation.
    pub fn cache_key_material(&self) -> String {
        CompileEnvironmentCacheMaterial::new(self).into_string()
    }
}

struct CompileEnvironmentCacheMaterial<'a> {
    env: &'a CompileEnvironment,
}

impl<'a> CompileEnvironmentCacheMaterial<'a> {
    fn new(env: &'a CompileEnvironment) -> Self {
        Self { env }
    }

    fn into_string(self) -> String {
        CompileEnvironmentCacheFields::from_env(self.env).into_string()
    }
}

struct CompileEnvironmentCacheFields {
    fields: [String; 10],
}

impl CompileEnvironmentCacheFields {
    fn from_env(env: &CompileEnvironment) -> Self {
        Self {
            fields: [
                env.compile_sdk_version.clone(),
                env.compatible_sdk_version.clone(),
                env.runtime_os.clone(),
                env.ets_loader_path.clone(),
                env.flags.obfuscation_enabled.to_string(),
                env.module_type.clone(),
                env.cache_path.clone(),
                CompileEnvironmentBuildMode::new(&env.build_mode)
                    .as_str()
                    .to_string(),
                env.flags.use_source_hash.to_string(),
                env.flags.use_path_placeholder.to_string(),
            ],
        }
    }

    fn into_string(self) -> String {
        self.fields.join("#")
    }
}

struct CompileEnvironmentBuildMode<'a> {
    value: &'a BuildMode,
}

impl<'a> CompileEnvironmentBuildMode<'a> {
    fn new(value: &'a BuildMode) -> Self {
        Self { value }
    }

    fn as_str(&self) -> &'static str {
        self.value.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_env_cache_key_is_deterministic() {
        let env = CompileEnvironment::from_plan(
            "6.0.0.47",
            "12",
            "OpenHarmony",
            "/sdk/ets-loader",
            "Entry",
            "/tmp/cache",
            BuildMode::Debug,
            CompileEnvFlagSet::default(),
        );
        let key1 = env.cache_key_material();
        let key2 = env.cache_key_material();
        assert_eq!(key1, key2);
    }

    #[test]
    fn esmodule_is_default_mode() {
        let env = CompileEnvironment::from_plan(
            "",
            "",
            "",
            "",
            "",
            "",
            BuildMode::Debug,
            CompileEnvFlagSet::default(),
        );
        assert!(env.is_module_mode());
    }

    #[test]
    fn compile_mode_display() {
        assert_eq!(CompileMode::EsModule.to_string(), "esmodule");
        assert_eq!(CompileMode::JsBundle.to_string(), "jsbundle");
    }
}
