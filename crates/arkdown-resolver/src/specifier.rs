//! Module specifier parsing and classification for OHOS resolution.
//!
//! Per SPEC-048: `specifier.rs` owns `ModuleSpecifier` parsing and
//! classification within the resolver context.

use std::path::Path;

/// Classification of a module import specifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecifierClass {
    /// Starts with `./` or `../` — relative file reference.
    Relative,
    /// `@bundle:`, `@shared:`, `@module:`, or `@normalized:` — OHM URL.
    OhmUrl,
    /// `@ohos/*`, `@kit.*`, or other scoped SDK/ohpm packages.
    ScopedPackage,
    /// Bare package name (no `@` prefix, no relative prefix).
    BarePackage,
    /// Absolute path — should not appear in normal source text.
    Absolute,
}

/// Classify a raw module specifier string.
///
/// Used by the resolver to route the specifier to the correct resolution
/// strategy without parsing it as a filesystem path.
pub fn classify_specifier(specifier: &str) -> SpecifierClass {
    if specifier.starts_with("./") || specifier.starts_with("../") {
        SpecifierClass::Relative
    } else if specifier.starts_with("@bundle:")
        || specifier.starts_with("@shared:")
        || specifier.starts_with("@module:")
        || specifier.starts_with("@normalized:")
    {
        SpecifierClass::OhmUrl
    } else if Path::new(specifier).is_absolute() {
        SpecifierClass::Absolute
    } else if specifier.starts_with('@') {
        SpecifierClass::ScopedPackage
    } else {
        SpecifierClass::BarePackage
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relative_dot_slash() {
        assert_eq!(
            classify_specifier("./pages/Index"),
            SpecifierClass::Relative
        );
    }

    #[test]
    fn test_relative_dot_dot() {
        assert_eq!(
            classify_specifier("../utils/helper"),
            SpecifierClass::Relative
        );
    }

    #[test]
    fn test_ohm_bundle() {
        assert_eq!(
            classify_specifier("@bundle:pkg/entry/ets/Index"),
            SpecifierClass::OhmUrl
        );
    }

    #[test]
    fn test_scoped_ohos_package() {
        assert_eq!(
            classify_specifier("@ohos/hilog"),
            SpecifierClass::ScopedPackage
        );
    }

    #[test]
    fn test_bare_package() {
        assert_eq!(classify_specifier("hilog"), SpecifierClass::BarePackage);
    }
}
