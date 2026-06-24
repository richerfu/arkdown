//! Module specifier parsing and classification for OHOS resolution.
//!
//! Per SPEC-048: `specifier.rs` owns `ModuleSpecifier` parsing and
//! classification within the resolver context.

use arkdown_path::{ModuleSpecifier, SpecifierKind};

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

impl SpecifierClass {
    /// Classify a raw module specifier string.
    ///
    /// Used by the resolver to route the specifier to the correct resolution
    /// strategy without parsing it as a filesystem path.
    pub fn classify(specifier: &str) -> Self {
        match ModuleSpecifier::new(specifier).kind() {
            SpecifierKind::Relative => Self::Relative,
            SpecifierKind::OhmUrl => Self::OhmUrl,
            SpecifierKind::ScopedPackage => Self::ScopedPackage,
            SpecifierKind::BarePackage => Self::BarePackage,
            SpecifierKind::AbsolutePath => Self::Absolute,
        }
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
            SpecifierClass::classify("./pages/Index"),
            SpecifierClass::Relative
        );
    }

    #[test]
    fn test_relative_dot_dot() {
        assert_eq!(
            SpecifierClass::classify("../utils/helper"),
            SpecifierClass::Relative
        );
    }

    #[test]
    fn test_ohm_bundle() {
        assert_eq!(
            SpecifierClass::classify("@bundle:pkg/entry/ets/Index"),
            SpecifierClass::OhmUrl
        );
    }

    #[test]
    fn test_scoped_ohos_package() {
        assert_eq!(
            SpecifierClass::classify("@ohos/hilog"),
            SpecifierClass::ScopedPackage
        );
    }

    #[test]
    fn test_bare_package() {
        assert_eq!(
            SpecifierClass::classify("hilog"),
            SpecifierClass::BarePackage
        );
    }
}
