//! Deprecated `@ohos:emitter` import rewriter.
//!
//! Rewrites imports from the legacy `@ohos:emitter` and `@ohos.util.emitter`
//! module paths to the canonical `@ohos.events.emitter` Stage-model path.
//!
//! # Upstream correspondence
//!
//! Mirrors hvigor's `ets-loader` `apiTransform` pass for emitter migration:
//! - `@ohos:emitter` → `@ohos.events.emitter`
//! - `@ohos.util.emitter` → `@ohos.events.emitter`
//! - `@system.emitter` → `@ohos.events.emitter`

/// Deprecated emitter specifiers → canonical Stage-model specifier.
static EMITTER_REWRITES: &[(&str, &str)] = &[
    ("@ohos:emitter", "@ohos.events.emitter"),
    ("@ohos.util.emitter", "@ohos.events.emitter"),
    ("@system.emitter", "@ohos.events.emitter"),
];

// ---------------------------------------------------------------------------
// EventEmitterImportRewriter
// ---------------------------------------------------------------------------

/// Rewrites legacy emitter import paths to `@ohos.events.emitter`.
///
/// # Example
///
/// ```text
/// // Input (deprecated)
/// import emitter from '@ohos:emitter';
///
/// // Output (Stage model)
/// import emitter from '@ohos.events.emitter';
/// ```
pub struct EventEmitterImportRewriter;

impl EventEmitterImportRewriter {
    /// Check whether `specifier` is a deprecated emitter module path.
    pub fn replacement_for(specifier: &str) -> Option<&'static str> {
        EMITTER_REWRITES
            .iter()
            .find(|(from, _)| *from == specifier)
            .map(|(_, to)| *to)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_table_matches_hvigor_api_transform() {
        for legacy in ["@ohos:emitter", "@ohos.util.emitter", "@system.emitter"] {
            assert_eq!(
                EventEmitterImportRewriter::replacement_for(legacy),
                Some("@ohos.events.emitter")
            );
        }
        assert_eq!(
            EventEmitterImportRewriter::replacement_for("@ohos.events.emitter"),
            None
        );
        assert_eq!(
            EventEmitterImportRewriter::replacement_for("@kit.NetworkKit"),
            None
        );
    }
}
