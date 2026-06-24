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

#[cfg(test)]
pub struct EventEmitterImportRewrite<'a> {
    source: &'a str,
}

impl EventEmitterImportRewriter {
    #[cfg(test)]
    pub fn new(source: &str) -> EventEmitterImportRewrite<'_> {
        EventEmitterImportRewrite { source }
    }

    /// Check whether `specifier` is a deprecated emitter module path.
    pub fn replacement_for(specifier: &str) -> Option<&'static str> {
        EMITTER_REWRITES
            .iter()
            .find(|(from, _)| *from == specifier)
            .map(|(_, to)| *to)
    }
}

#[cfg(test)]
impl<'a> EventEmitterImportRewrite<'a> {
    /// Rewrite deprecated emitter imports in `source`.
    ///
    /// Returns the source unchanged if no deprecated emitter imports are found.
    pub fn rewrite(&self) -> std::borrow::Cow<'a, str> {
        if !self.source.contains("@ohos:emitter")
            && !self.source.contains("@ohos.util.emitter")
            && !self.source.contains("@system.emitter")
        {
            return std::borrow::Cow::Borrowed(self.source);
        }
        crate::StaticModuleSpecifierRewrite::new(
            self.source,
            "arkdown-emitter.ets",
            |specifier: &str| {
                EventEmitterImportRewriter::replacement_for(specifier).map(|r| r.to_string())
            },
        )
        .rewrite()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_ohos_colon_emitter() {
        let src = "import emitter from '@ohos:emitter';";
        let out = EventEmitterImportRewriter::new(src).rewrite();
        assert!(
            out.contains("@ohos.events.emitter"),
            "@ohos:emitter → @ohos.events.emitter: {out}"
        );
        assert!(
            !out.contains("@ohos:emitter"),
            "old specifier removed: {out}"
        );
    }

    #[test]
    fn rewrites_ohos_util_emitter() {
        let src = "import { on } from '@ohos.util.emitter';";
        let out = EventEmitterImportRewriter::new(src).rewrite();
        assert!(
            out.contains("@ohos.events.emitter"),
            "util.emitter → events.emitter: {out}"
        );
    }

    #[test]
    fn rewrites_system_emitter() {
        let src = "import emitter from '@system.emitter';";
        let out = EventEmitterImportRewriter::new(src).rewrite();
        assert!(
            out.contains("@ohos.events.emitter"),
            "system.emitter → events.emitter: {out}"
        );
    }

    #[test]
    fn preserves_canonical_emitter_import() {
        let src = "import emitter from '@ohos.events.emitter';";
        let out = EventEmitterImportRewriter::new(src).rewrite();
        assert_eq!(
            out.trim(),
            src.trim(),
            "canonical import must not be rewritten"
        );
    }

    #[test]
    fn preserves_unrelated_imports() {
        let src = "import { on } from '@kit.NetworkKit';";
        let out = EventEmitterImportRewriter::new(src).rewrite();
        assert_eq!(
            out.trim(),
            src.trim(),
            "unrelated import must not be changed"
        );
    }

    #[test]
    fn replacement_for_returns_correct_mapping() {
        assert_eq!(
            EventEmitterImportRewriter::replacement_for("@ohos:emitter"),
            Some("@ohos.events.emitter")
        );
        assert_eq!(
            EventEmitterImportRewriter::replacement_for("@ohos.events.emitter"),
            None
        );
    }
}
