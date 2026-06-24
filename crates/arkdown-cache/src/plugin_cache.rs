//! Plugin cache invalidation bridge.
//!
//! hvigor uses a `shouldInvalidCache` plugin hook to let individual plugins
//! contribute to cache invalidation decisions.  This module provides the types
//! to bridge plugin-reported decisions into the unified stage cache key.
//!
//! # Upstream correspondence
//!
//! | Type | hvigor equivalent |
//! |------|-------------------|
//! | `ShouldInvalidCacheBridge` | `arkts-pack.js::wrapperShouldInvalidCache` |
//! | `PluginInvalidationInput` | arguments passed to each plugin hook |
//! | `PluginInvalidationOutput` | result returned by a plugin hook |
//! | `PluginCacheDecision` | merged decision across all plugins |

use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// PluginInvalidationInput
// ---------------------------------------------------------------------------

/// Input passed to each plugin's `shouldInvalidCache` hook.
///
/// # Upstream correspondence
///
/// Corresponds to the argument object passed to `wrapperShouldInvalidCache`
/// in `arkts-pack.js`, which includes the compile env, changed files, and
/// per-module metadata.
#[derive(Debug, Clone)]
pub struct PluginInvalidationInput {
    /// The current compile environment key string.
    pub compile_env: String,
    /// Set of file paths that have changed since the last build.
    pub changed_files: Vec<String>,
    /// Module ID (OHM URL or absolute path) for per-module decisions.
    pub module_id: Option<String>,
    /// Whether this is a watch-mode (incremental) build.
    pub is_watch: bool,
}

// ---------------------------------------------------------------------------
// PluginInvalidationOutput
// ---------------------------------------------------------------------------

/// Result returned by a plugin's `shouldInvalidCache` hook.
///
/// # Upstream correspondence
///
/// Corresponds to the boolean result and optional reason string returned by
/// plugins in `invalid-cache-plugin.js`.
#[derive(Debug, Clone)]
pub struct PluginInvalidationOutput {
    /// Whether this plugin requests cache invalidation.
    pub should_invalidate: bool,
    /// Optional reason for the invalidation decision.
    pub reason: Option<String>,
    /// Whether this plugin forces ALL modules to be invalidated (global
    /// invalidation).  Corresponds to hvigor's global cache reset on
    /// compile-env change.
    pub global_invalidation: bool,
}

impl PluginInvalidationOutput {
    /// A decision to NOT invalidate, with no reason.
    pub fn keep() -> Self {
        Self {
            should_invalidate: false,
            reason: None,
            global_invalidation: false,
        }
    }

    /// A decision to invalidate with a human-readable reason.
    pub fn invalidate(reason: impl Into<String>) -> Self {
        Self {
            should_invalidate: true,
            reason: Some(reason.into()),
            global_invalidation: false,
        }
    }

    /// A decision to invalidate ALL modules globally.
    ///
    /// # Upstream correspondence
    ///
    /// Corresponds to hvigor's compile-env change, which resets the entire
    /// cache.
    pub fn invalidate_all(reason: impl Into<String>) -> Self {
        Self {
            should_invalidate: true,
            reason: Some(reason.into()),
            global_invalidation: true,
        }
    }
}

// ---------------------------------------------------------------------------
// PluginCacheDecision
// ---------------------------------------------------------------------------

/// Merged cache invalidation decision across all plugins.
///
/// When ANY plugin returns `should_invalidate: true`, the merged decision is
/// to invalidate the cache entry.  When ANY plugin returns
/// `global_invalidation: true`, the decision applies to ALL module entries.
///
/// # Upstream correspondence
///
/// Corresponds to the merged result of calling `wrapperShouldInvalidCache`
/// across all registered plugins in `arkts-pack.js`.
#[derive(Debug, Clone, Default)]
pub struct PluginCacheDecision {
    /// Whether any plugin voted for invalidation.
    pub should_invalidate: bool,
    /// Whether any plugin voted for global (all-module) invalidation.
    pub global_invalidation: bool,
    /// Map from plugin name → reason, for diagnostics.
    pub reasons: BTreeMap<String, String>,
}

impl PluginCacheDecision {
    /// Merge a single plugin's output into this decision.
    pub fn merge(&mut self, plugin_name: &str, output: PluginInvalidationOutput) {
        if output.should_invalidate {
            self.should_invalidate = true;
        }
        if output.global_invalidation {
            self.global_invalidation = true;
        }
        if let Some(reason) = output.reason {
            self.reasons.insert(plugin_name.to_string(), reason);
        }
    }

    /// Force the decision to invalidate.
    ///
    /// Used when the compile-env check determines that ALL cache entries must
    /// be discarded.
    pub fn force_invalidate(&mut self, reason: impl Into<String>) {
        self.should_invalidate = true;
        self.global_invalidation = true;
        self.reasons.insert("__system__".to_string(), reason.into());
    }

    /// Return a human-readable explanation of why invalidation was requested.
    pub fn explain(&self) -> String {
        if self.reasons.is_empty() {
            return "no reason recorded".to_string();
        }
        self.reasons
            .iter()
            .map(|(plugin, reason)| format!("{plugin}: {reason}"))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

// ---------------------------------------------------------------------------
// ShouldInvalidCacheBridge
// ---------------------------------------------------------------------------

/// Aggregates plugin cache invalidation decisions.
///
/// `ShouldInvalidCacheBridge` collects outputs from multiple plugin
/// `shouldInvalidCache` hook invocations and merges them into a single
/// [`PluginCacheDecision`].
///
/// # Upstream correspondence
///
/// Corresponds to `arkts-pack.js::wrapperShouldInvalidCache`, which calls
/// each registered plugin hook and merges the results.
pub struct ShouldInvalidCacheBridge {
    decision: PluginCacheDecision,
}

impl ShouldInvalidCacheBridge {
    /// Create a new bridge with an initial "keep" decision.
    pub fn new() -> Self {
        Self {
            decision: PluginCacheDecision::default(),
        }
    }

    /// Record a plugin's `shouldInvalidCache` decision.
    ///
    /// `plugin_name` is used only for diagnostic messages.
    pub fn ask_plugin(&mut self, plugin_name: &str, output: PluginInvalidationOutput) {
        self.decision.merge(plugin_name, output);
    }

    /// Merge multiple plugin decisions in one call.
    pub fn merge_decisions(
        &mut self,
        decisions: impl IntoIterator<Item = (String, PluginInvalidationOutput)>,
    ) {
        for (name, output) in decisions {
            self.decision.merge(&name, output);
        }
    }

    /// Force global invalidation regardless of plugin decisions.
    ///
    /// Call this when the compile-env has changed.
    pub fn force_invalidate(&mut self, reason: impl Into<String>) {
        self.decision.force_invalidate(reason);
    }

    /// Return the merged decision.
    pub fn finish(self) -> PluginCacheDecision {
        self.decision
    }
}

impl Default for ShouldInvalidCacheBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_decision_not_invalidated() {
        let mut bridge = ShouldInvalidCacheBridge::new();
        bridge.ask_plugin("ets-transform", PluginInvalidationOutput::keep());
        let decision = bridge.finish();
        assert!(!decision.should_invalidate);
        assert!(!decision.global_invalidation);
    }

    #[test]
    fn one_invalidating_plugin_overrides_keep() {
        let mut bridge = ShouldInvalidCacheBridge::new();
        bridge.ask_plugin("ets-transform", PluginInvalidationOutput::keep());
        bridge.ask_plugin(
            "abc-compiler",
            PluginInvalidationOutput::invalidate("source changed"),
        );
        let decision = bridge.finish();
        assert!(decision.should_invalidate);
        assert!(!decision.global_invalidation);
        assert!(decision.explain().contains("source changed"));
    }

    #[test]
    fn global_invalidation_marks_global() {
        let mut bridge = ShouldInvalidCacheBridge::new();
        bridge.force_invalidate("compile env changed");
        let decision = bridge.finish();
        assert!(decision.should_invalidate);
        assert!(decision.global_invalidation);
    }
}
