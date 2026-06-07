//! [`ModuleSpecifier`] — a raw import/export specifier from source text.
//!
//! A specifier is NOT a filesystem path until a resolver decides it is. It can be:
//! - A relative path: `"./pages/Index"`, `"../utils/helper"`
//! - A bare package name: `"@ohos/hilog"`, `"@kit.UIAbility"`
//! - An OHM URL: `"@bundle:pkg/entry/ets/Index"`
//! - A file path (when `@module:` or `@normalized:` prefixes are resolved)

use std::fmt;

use serde::{Deserialize, Serialize};

/// A raw module specifier as written in source text.
///
/// This type carries no resolution semantics. Use it to pass specifiers
/// through APIs without accidentally treating them as filesystem paths.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModuleSpecifier(String);

/// The kind of module specifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecifierKind {
    /// Starts with `./` or `../` — relative file reference.
    Relative,
    /// Starts with `@bundle:`, `@shared:`, `@module:`, or `@normalized:`.
    OhmUrl,
    /// Starts with `@` but not an OHM prefix — scoped npm/ohpm package.
    ScopedPackage,
    /// No prefix — bare package name like `"hilog"`.
    BarePackage,
    /// Absolute path — should not appear in normal source text.
    AbsolutePath,
}

impl ModuleSpecifier {
    /// Wrap a raw specifier string.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Borrow as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume into the inner `String`.
    pub fn into_string(self) -> String {
        self.0
    }

    /// Classify the kind of this specifier.
    pub fn kind(&self) -> SpecifierKind {
        let s = self.0.as_str();
        if s.starts_with("./") || s.starts_with("../") {
            SpecifierKind::Relative
        } else if s.starts_with("@bundle:")
            || s.starts_with("@shared:")
            || s.starts_with("@module:")
            || s.starts_with("@normalized:")
        {
            SpecifierKind::OhmUrl
        } else if std::path::Path::new(s).is_absolute() {
            SpecifierKind::AbsolutePath
        } else if s.starts_with('@') {
            SpecifierKind::ScopedPackage
        } else {
            SpecifierKind::BarePackage
        }
    }

    /// Return `true` if this specifier is relative (`./` or `../`).
    pub fn is_relative(&self) -> bool {
        matches!(self.kind(), SpecifierKind::Relative)
    }

    /// Return `true` if this specifier is an OHM URL.
    pub fn is_ohm_url(&self) -> bool {
        matches!(self.kind(), SpecifierKind::OhmUrl)
    }
}

impl fmt::Display for ModuleSpecifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ModuleSpecifier {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for ModuleSpecifier {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ModuleSpecifier {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specifier_relative_dot_slash() {
        let s = ModuleSpecifier::new("./pages/Index");
        assert!(s.is_relative());
        assert!(!s.is_ohm_url());
    }

    #[test]
    fn test_specifier_relative_dot_dot() {
        let s = ModuleSpecifier::new("../utils/helper");
        assert!(s.is_relative());
    }

    #[test]
    fn test_specifier_scoped_package() {
        let s = ModuleSpecifier::new("@ohos/hilog");
        assert_eq!(s.kind(), SpecifierKind::ScopedPackage);
        assert!(!s.is_relative());
        assert!(!s.is_ohm_url());
    }

    #[test]
    fn test_specifier_bare_package() {
        let s = ModuleSpecifier::new("hilog");
        assert_eq!(s.kind(), SpecifierKind::BarePackage);
    }

    #[test]
    fn test_specifier_ohm_bundle() {
        let s = ModuleSpecifier::new("@bundle:pkg/entry/ets/Index");
        assert!(s.is_ohm_url());
        assert!(!s.is_relative());
    }

    #[test]
    fn test_specifier_ohm_shared() {
        let s = ModuleSpecifier::new("@shared:hsp/ets/pages/Index");
        assert!(s.is_ohm_url());
    }

    #[test]
    fn test_specifier_kit_prefix_is_scoped_package() {
        let s = ModuleSpecifier::new("@kit.UIAbility");
        assert_eq!(s.kind(), SpecifierKind::ScopedPackage);
    }
}
