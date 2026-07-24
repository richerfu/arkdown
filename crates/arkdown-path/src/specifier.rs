//! [`ModuleSpecifier`] — a raw import/export specifier from source text.
//!
//! A specifier is NOT a filesystem path until a resolver decides it is. It can be:
//! - A relative path: `"./pages/Index"`, `"../utils/helper"`
//! - A bare package name: `"@ohos/hilog"`, `"@kit.UIAbility"`
//! - An OHM URL: `"@bundle:pkg/entry/ets/Index"`
//! - A file path (when `@module:` or `@normalized:` prefixes are resolved)

use std::{fmt, path::Path};

use serde::{Deserialize, Serialize};

use crate::serializer::ArtifactPathSerializer;

/// A raw module specifier as written in source text.
///
/// This type carries no resolution semantics. Use it to pass specifiers
/// through APIs without accidentally treating them as filesystem paths.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModuleSpecifier(String);

/// Borrowed module specifier text.
///
/// Use this in transform and resolver hot paths when the source text already
/// owns the specifier and no persistent owned key is needed.
#[derive(Debug, Clone, Copy)]
pub struct ModuleSpecifierText<'a> {
    raw: &'a str,
}

/// Candidate package root and remaining subpath for a package-like specifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSpecifierCandidate {
    package_name: String,
    subpath: String,
}

/// The kind of module specifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecifierKind {
    /// Starts with `./` / `../`, or is `.` / `..` — relative file reference.
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

    /// Build a relative module specifier from a path relative to the importing
    /// source root.
    pub fn from_relative_path(path: &Path) -> Self {
        let rel = ArtifactPathSerializer::rel_to_slash(path);
        Self(["./", rel.as_str()].concat())
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
        ModuleSpecifierText::new(self.as_str()).kind()
    }

    /// Return `true` if this specifier is relative (`./` or `../`).
    pub fn is_relative(&self) -> bool {
        ModuleSpecifierText::new(self.as_str()).is_relative()
    }

    /// Return `true` if this specifier is an OHM URL.
    pub fn is_ohm_url(&self) -> bool {
        ModuleSpecifierText::new(self.as_str()).is_ohm_url()
    }

    /// Return `true` for OHOS/HMS/system/native runtime module namespaces that
    /// are provided by the platform and should be externalized by bundler
    /// plugins instead of resolved as files.
    pub fn is_runtime_builtin(&self) -> bool {
        ModuleSpecifierText::new(self.as_str()).is_runtime_builtin()
    }

    /// Return `true` for SDK declaration package specifiers resolved from
    /// SDK `api/` or `kits/` directories.
    pub fn is_sdk_declaration_specifier(&self) -> bool {
        ModuleSpecifierText::new(self.as_str()).is_sdk_declaration_specifier()
    }

    /// Return the primary SDK `api/` declaration file name for `@ohos/<api>` imports.
    pub fn sdk_api_filename(&self) -> Option<String> {
        ModuleSpecifierText::new(self.as_str()).sdk_api_filename()
    }

    /// Return SDK `api/` declaration file candidates for `@ohos/<api>` imports.
    pub fn sdk_api_filenames(&self) -> Vec<String> {
        ModuleSpecifierText::new(self.as_str()).sdk_api_filenames()
    }

    /// Return the primary SDK `kits/` declaration file name for `@kit.*` imports.
    pub fn sdk_kit_filename(&self) -> Option<String> {
        ModuleSpecifierText::new(self.as_str()).sdk_kit_filename()
    }

    /// Return SDK `kits/` declaration file candidates for `@kit.*` imports.
    pub fn sdk_kit_filenames(&self) -> Vec<String> {
        ModuleSpecifierText::new(self.as_str()).sdk_kit_filenames()
    }

    /// Return SDK `arkts/` declaration file candidates for `@arkts.*` imports.
    pub fn sdk_arkts_filenames(&self) -> Vec<String> {
        ModuleSpecifierText::new(self.as_str()).sdk_arkts_filenames()
    }

    /// Return SDK API declaration candidates for runtime-form `@ohos:` and
    /// `@hms:` specifiers. Bundlers still treat these specifiers as platform
    /// externals; this mapping is for compiler type metadata lookup only.
    pub fn sdk_runtime_declaration_filenames(&self) -> Vec<String> {
        ModuleSpecifierText::new(self.as_str()).sdk_runtime_declaration_filenames()
    }

    /// Return `true` for `@kit.*` SDK imports.
    pub fn is_kit(&self) -> bool {
        ModuleSpecifierText::new(self.as_str()).is_kit()
    }

    /// Return `true` when a package-like specifier mentions the module name in
    /// the first or last normal segment, matching hvigor's package-name checks.
    pub fn matches_package_module_name(&self, module_name: &str) -> bool {
        ModuleSpecifierText::new(self.as_str()).matches_package_module_name(module_name)
    }

    /// Return the module-name candidate from the last normal package segment.
    pub fn package_module_name_candidate(&self) -> Option<&str> {
        ModuleSpecifierText::new(self.as_str()).package_module_name_candidate()
    }

    /// Convert legacy `@ohos/<name>` imports into `@system.<name>` when hvigor
    /// compatibility requires the pre-API-9 runtime namespace.
    pub fn legacy_system_specifier(&self) -> Option<String> {
        ModuleSpecifierText::new(self.as_str()).legacy_system_specifier()
    }

    /// Return package-root candidates for bare and scoped package specifiers.
    pub fn package_candidates(&self) -> Vec<PackageSpecifierCandidate> {
        ModuleSpecifierText::new(self.as_str()).package_candidates()
    }
}

impl<'a> ModuleSpecifierText<'a> {
    /// Borrow raw module specifier source text.
    pub fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    /// Borrow as a string slice.
    pub fn as_str(&self) -> &'a str {
        self.raw
    }

    /// Classify the kind of this specifier.
    pub fn kind(&self) -> SpecifierKind {
        if self.raw == "."
            || self.raw == ".."
            || self.raw.starts_with("./")
            || self.raw.starts_with("../")
        {
            SpecifierKind::Relative
        } else if self.raw.starts_with("@bundle:")
            || self.raw.starts_with("@shared:")
            || self.raw.starts_with("@module:")
            || self.raw.starts_with("@normalized:")
        {
            SpecifierKind::OhmUrl
        } else if Path::new(self.raw).is_absolute() {
            SpecifierKind::AbsolutePath
        } else if self.raw.starts_with('@') {
            SpecifierKind::ScopedPackage
        } else {
            SpecifierKind::BarePackage
        }
    }

    /// Return `true` if this specifier is relative (`./`, `../`, `.`, or `..`).
    pub fn is_relative(&self) -> bool {
        matches!(self.kind(), SpecifierKind::Relative)
    }

    /// Return `true` if this specifier is an OHM URL.
    pub fn is_ohm_url(&self) -> bool {
        matches!(self.kind(), SpecifierKind::OhmUrl)
    }

    /// Return `true` for OHOS/HMS/system/native runtime module namespaces.
    pub fn is_runtime_builtin(&self) -> bool {
        RuntimeBuiltinSpecifier::new(self.raw).is_match()
    }

    /// Return `true` for SDK declaration package specifiers resolved from
    /// SDK `api/` or `kits/` directories.
    pub fn is_sdk_declaration_specifier(&self) -> bool {
        SdkDeclarationSpecifier::new(self.raw).is_match()
    }

    /// Return the primary SDK `api/` declaration file name for `@ohos/<api>` imports.
    pub fn sdk_api_filename(&self) -> Option<String> {
        SdkDeclarationSpecifier::new(self.raw)
            .api_filenames()
            .into_iter()
            .next()
    }

    /// Return SDK `api/` declaration file candidates for `@ohos/<api>` imports.
    pub fn sdk_api_filenames(&self) -> Vec<String> {
        SdkDeclarationSpecifier::new(self.raw).api_filenames()
    }

    /// Return the primary SDK `kits/` declaration file name for `@kit.*` imports.
    pub fn sdk_kit_filename(&self) -> Option<String> {
        SdkDeclarationSpecifier::new(self.raw)
            .kit_filenames()
            .into_iter()
            .next()
    }

    /// Return SDK `kits/` declaration file candidates for `@kit.*` imports.
    pub fn sdk_kit_filenames(&self) -> Vec<String> {
        SdkDeclarationSpecifier::new(self.raw).kit_filenames()
    }

    /// Return SDK `arkts/` declaration file candidates for `@arkts.*` imports.
    pub fn sdk_arkts_filenames(&self) -> Vec<String> {
        SdkDeclarationSpecifier::new(self.raw).arkts_filenames()
    }

    /// Return SDK API declaration candidates for runtime-form `@ohos:` and
    /// `@hms:` specifiers without changing their external-module kind.
    pub fn sdk_runtime_declaration_filenames(&self) -> Vec<String> {
        SdkDeclarationSpecifier::new(self.raw).runtime_filenames()
    }

    /// Return `true` for `@kit.*` SDK imports.
    pub fn is_kit(&self) -> bool {
        KitSpecifier::new(self.raw).is_match()
    }

    /// Return `true` when a package-like specifier mentions the module name in
    /// the first or last normal segment, matching hvigor's package-name checks.
    pub fn matches_package_module_name(&self, module_name: &str) -> bool {
        PackageModuleSpecifier::new(self.raw).matches_module_name(module_name)
    }

    /// Return the last normal package segment as the module-name candidate.
    pub fn package_module_name_candidate(&self) -> Option<&'a str> {
        PackageModuleSpecifier::new(self.raw).module_name_candidate()
    }

    /// Convert legacy `@ohos/<name>` imports into `@system.<name>` when hvigor
    /// compatibility requires the pre-API-9 runtime namespace.
    pub fn legacy_system_specifier(&self) -> Option<String> {
        LegacySystemSpecifier::new(self.raw).specifier()
    }

    /// Return package-root candidates for bare and scoped package specifiers.
    pub fn package_candidates(&self) -> Vec<PackageSpecifierCandidate> {
        PackageCandidateList::new(self.raw, self.kind()).candidates()
    }
}

impl PackageSpecifierCandidate {
    /// Package name candidate, for example `@scope/pkg` or `pkg`.
    pub fn package_name(&self) -> &str {
        &self.package_name
    }

    /// Remaining subpath inside the package root.
    pub fn subpath(&self) -> &str {
        &self.subpath
    }
}

struct RuntimeBuiltinSpecifier<'a> {
    raw: &'a str,
}

impl<'a> RuntimeBuiltinSpecifier<'a> {
    const OHOS_COLON_PREFIX: &'static str = "@ohos:";
    const HMS_COLON_PREFIX: &'static str = "@hms:";
    const HMS_DOT_PREFIX: &'static str = "@hms.";
    const SYSTEM_DOT_PREFIX: &'static str = "@system.";
    const NATIVE_COLON_PREFIX: &'static str = "@native:";

    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn is_match(&self) -> bool {
        self.raw.starts_with(Self::OHOS_COLON_PREFIX)
            || self.raw.starts_with(Self::HMS_COLON_PREFIX)
            || self.raw.starts_with(Self::HMS_DOT_PREFIX)
            || self.raw.starts_with(Self::SYSTEM_DOT_PREFIX)
            || self.raw.starts_with(Self::NATIVE_COLON_PREFIX)
    }
}

struct SdkDeclarationSpecifier<'a> {
    raw: &'a str,
}

struct KitSpecifier<'a> {
    raw: &'a str,
}

impl<'a> KitSpecifier<'a> {
    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn is_match(&self) -> bool {
        SdkDeclarationSpecifier::new(self.raw)
            .kit_filenames()
            .into_iter()
            .next()
            .is_some()
    }
}

struct PackageModuleSpecifier<'a> {
    raw: &'a str,
}

impl<'a> PackageModuleSpecifier<'a> {
    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn matches_module_name(&self, module_name: &str) -> bool {
        let mut first = None;
        let mut last = None;
        for segment in self.normal_segments() {
            first.get_or_insert(segment);
            last = Some(segment);
        }
        first.is_some_and(|segment| segment == module_name)
            || last.is_some_and(|segment| segment == module_name)
    }

    fn module_name_candidate(&self) -> Option<&'a str> {
        self.normal_segments().last()
    }

    fn normal_segments(&self) -> impl Iterator<Item = &'a str> {
        self.normalized()
            .split('/')
            .filter(|segment| !segment.is_empty() && *segment != "." && *segment != "..")
    }

    fn normalized(&self) -> &'a str {
        self.raw
            .strip_prefix(SdkDeclarationSpecifier::OHOS_SLASH_PREFIX)
            .or_else(|| {
                self.raw
                    .strip_prefix(RuntimeBuiltinSpecifier::OHOS_COLON_PREFIX)
            })
            .unwrap_or(self.raw)
    }
}

struct LegacySystemSpecifier<'a> {
    raw: &'a str,
}

impl<'a> LegacySystemSpecifier<'a> {
    const SYSTEM_PREFIX: &'static str = "@system.";

    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn specifier(&self) -> Option<String> {
        let suffix = self
            .raw
            .strip_prefix(SdkDeclarationSpecifier::OHOS_SLASH_PREFIX)?;
        Some([Self::SYSTEM_PREFIX, suffix].concat())
    }
}

struct PackageCandidateList<'a> {
    raw: &'a str,
    kind: SpecifierKind,
}

impl<'a> PackageCandidateList<'a> {
    fn new(raw: &'a str, kind: SpecifierKind) -> Self {
        Self { raw, kind }
    }

    fn candidates(&self) -> Vec<PackageSpecifierCandidate> {
        let min_parts = match self.kind {
            SpecifierKind::BarePackage => 1,
            SpecifierKind::ScopedPackage => 2,
            SpecifierKind::Relative | SpecifierKind::OhmUrl | SpecifierKind::AbsolutePath => {
                return Vec::new();
            }
        };
        let parts = self.raw.split('/').collect::<Vec<_>>();
        if parts.len() < min_parts {
            return Vec::new();
        }

        let mut candidates = Vec::new();
        for end in (min_parts..=parts.len()).rev() {
            candidates.push(PackageSpecifierCandidate {
                package_name: parts[..end].join("/"),
                subpath: parts[end..].join("/"),
            });
        }
        candidates
    }
}

impl<'a> SdkDeclarationSpecifier<'a> {
    const OHOS_SLASH_PREFIX: &'static str = "@ohos/";
    const OHOS_DOT_PREFIX: &'static str = "@ohos.";
    const OHOS_COLON_PREFIX: &'static str = "@ohos:";
    const HMS_COLON_PREFIX: &'static str = "@hms:";
    const KIT_PREFIX: &'static str = "@kit.";
    const ARKTS_DOT_PREFIX: &'static str = "@arkts.";

    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn is_match(&self) -> bool {
        self.raw.starts_with(Self::OHOS_SLASH_PREFIX)
            || self.raw.starts_with(Self::OHOS_DOT_PREFIX)
            || self.raw.starts_with(Self::KIT_PREFIX)
            || self.raw.starts_with(Self::ARKTS_DOT_PREFIX)
    }

    fn api_filenames(&self) -> Vec<String> {
        if let Some(api_name) = self.raw.strip_prefix(Self::OHOS_SLASH_PREFIX) {
            return Self::declaration_filenames("@ohos.", api_name);
        }
        if self.raw.starts_with(Self::OHOS_DOT_PREFIX) {
            return Self::declaration_filenames("", self.raw);
        }
        Vec::new()
    }

    fn kit_filenames(&self) -> Vec<String> {
        if self.raw.starts_with(Self::KIT_PREFIX) {
            return Self::declaration_filenames("", self.raw);
        }
        Vec::new()
    }

    fn arkts_filenames(&self) -> Vec<String> {
        if self.raw.starts_with(Self::ARKTS_DOT_PREFIX) {
            return Self::declaration_filenames("", self.raw);
        }
        Vec::new()
    }

    fn runtime_filenames(&self) -> Vec<String> {
        if let Some(api_name) = self.raw.strip_prefix(Self::OHOS_COLON_PREFIX) {
            return Self::declaration_filenames(Self::OHOS_DOT_PREFIX, api_name);
        }
        if let Some(api_name) = self.raw.strip_prefix(Self::HMS_COLON_PREFIX) {
            return Self::declaration_filenames("@hms.", api_name);
        }
        Vec::new()
    }

    fn declaration_filenames(prefix: &str, name: &str) -> Vec<String> {
        vec![
            Self::declaration_filename(prefix, name, ".d.ts"),
            Self::declaration_filename(prefix, name, ".d.ets"),
        ]
    }

    fn declaration_filename(prefix: &str, name: &str, extension: &str) -> String {
        let mut file_name = String::with_capacity(prefix.len() + name.len() + extension.len());
        file_name.push_str(prefix);
        file_name.push_str(name);
        file_name.push_str(extension);
        file_name
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
    fn test_specifier_relative_directory_self() {
        let s = ModuleSpecifier::new(".");
        assert_eq!(s.kind(), SpecifierKind::Relative);
        assert!(s.is_relative());
    }

    #[test]
    fn test_specifier_relative_directory_parent() {
        let s = ModuleSpecifier::new("..");
        assert_eq!(s.kind(), SpecifierKind::Relative);
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

    #[test]
    fn test_runtime_builtin_specifiers() {
        for raw in [
            "@ohos:hilog",
            "@hms:push",
            "@hms.account",
            "@system.router",
            "@native:libentry.so",
        ] {
            assert!(ModuleSpecifier::new(raw).is_runtime_builtin(), "{raw}");
        }
        assert!(!ModuleSpecifier::new("@ohos/hilog").is_runtime_builtin());
    }

    #[test]
    fn test_sdk_declaration_specifiers() {
        assert!(ModuleSpecifier::new("@ohos/hilog").is_sdk_declaration_specifier());
        assert!(ModuleSpecifier::new("@ohos.hilog").is_sdk_declaration_specifier());
        assert!(ModuleSpecifier::new("@kit.ArkUI").is_sdk_declaration_specifier());
        assert!(ModuleSpecifier::new("@arkts.utils").is_sdk_declaration_specifier());
        assert!(!ModuleSpecifier::new("@ohos:hilog").is_sdk_declaration_specifier());
        assert!(!ModuleSpecifier::new("entry").is_sdk_declaration_specifier());
    }

    #[test]
    fn test_sdk_declaration_filenames() {
        assert_eq!(
            ModuleSpecifier::new("@ohos/hilog").sdk_api_filename(),
            Some("@ohos.hilog.d.ts".to_string())
        );
        assert_eq!(
            ModuleSpecifier::new("@ohos.file.PhotoPickerComponent").sdk_api_filenames(),
            vec![
                "@ohos.file.PhotoPickerComponent.d.ts".to_string(),
                "@ohos.file.PhotoPickerComponent.d.ets".to_string()
            ]
        );
        assert_eq!(
            ModuleSpecifier::new("@ohos.data.preferences").sdk_api_filename(),
            Some("@ohos.data.preferences.d.ts".to_string())
        );
        assert_eq!(
            ModuleSpecifier::new("@kit.ArkUI").sdk_kit_filename(),
            Some("@kit.ArkUI.d.ts".to_string())
        );
        assert_eq!(
            ModuleSpecifier::new("@arkts.utils").sdk_arkts_filenames(),
            vec![
                "@arkts.utils.d.ts".to_string(),
                "@arkts.utils.d.ets".to_string()
            ]
        );
        assert_eq!(ModuleSpecifier::new("@ohos:hilog").sdk_api_filename(), None);
        assert_eq!(ModuleSpecifier::new("@ohos/hilog").sdk_kit_filename(), None);
        assert_eq!(
            ModuleSpecifier::new("@ohos:multimedia.media").sdk_runtime_declaration_filenames(),
            vec![
                "@ohos.multimedia.media.d.ts".to_string(),
                "@ohos.multimedia.media.d.ets".to_string()
            ]
        );
        assert_eq!(
            ModuleSpecifier::new("@hms:ai.textToSpeech").sdk_runtime_declaration_filenames(),
            vec![
                "@hms.ai.textToSpeech.d.ts".to_string(),
                "@hms.ai.textToSpeech.d.ets".to_string()
            ]
        );
    }
}
