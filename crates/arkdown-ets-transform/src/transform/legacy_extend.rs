use std::{borrow::Cow, collections::BTreeSet, sync::OnceLock};

use arkdown_core::ArktsLanguageVersion;
use regex::{Captures, Regex};

/// Exact ArkTS 1.1 Extend preprocessing used before both TypeScript checking
/// and source transformation.
///
/// Source owner: `validate_ui_syntax.ts::preprocessExtend` and
/// `preprocessNewExtend`. Those functions deliberately use global source-text
/// regular expressions; they do not parse comments or string literals first.
pub struct ArktsExtendSource<'a> {
    source: Cow<'a, str>,
    legacy_names: BTreeSet<String>,
    modern_names: BTreeSet<String>,
}

impl<'a> ArktsExtendSource<'a> {
    pub fn new(source: &'a str, version: ArktsLanguageVersion) -> Self {
        if version.is_static() || !source.contains("@Extend") {
            return Self {
                source: Cow::Borrowed(source),
                legacy_names: BTreeSet::new(),
                modern_names: BTreeSet::new(),
            };
        }

        let legacy_pattern = Self::legacy_pattern();
        let mut legacy_names = BTreeSet::new();
        let normalized = legacy_pattern.replace_all(source, |captures: &Captures<'_>| {
            let whitespace = captures.get(1).expect("Extend whitespace capture").as_str();
            let component = captures.get(2).expect("Extend component capture").as_str();
            let name = captures.get(3).expect("Extend name capture").as_str();
            legacy_names.insert(name.to_string());
            format!("@Extend({component}){whitespace}function __{component}__{name}(")
        });
        let mut modern_names = BTreeSet::new();
        for captures in Self::modern_pattern().captures_iter(normalized.as_ref()) {
            modern_names.insert(
                captures
                    .get(1)
                    .expect("modern Extend name capture")
                    .as_str()
                    .to_string(),
            );
        }
        Self {
            source: normalized,
            legacy_names,
            modern_names,
        }
    }

    pub fn as_str(&self) -> &str {
        self.source.as_ref()
    }

    pub fn legacy_names(&self) -> &BTreeSet<String> {
        &self.legacy_names
    }

    pub fn modern_names(&self) -> &BTreeSet<String> {
        &self.modern_names
    }

    fn legacy_pattern() -> &'static Regex {
        static PATTERN: OnceLock<Regex> = OnceLock::new();
        PATTERN.get_or_init(|| {
            Regex::new(r"@Extend(\s+)([^\.\s]+)\.([^\(]+)\(")
                .expect("upstream Extend pattern is valid")
        })
    }

    fn modern_pattern() -> &'static Regex {
        static PATTERN: OnceLock<Regex> = OnceLock::new();
        PATTERN.get_or_init(|| {
            Regex::new(r"@Extend\s*\([^\)]+\)\s*function\s+([^\(\s]+)\s*\(")
                .expect("upstream modern Extend pattern is valid")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_original_extend_declarations_only_in_arkts_11() {
        let source = "  @Extend Text.fancy(color:string){\n    .fontColor(color)\n  }\n";
        assert_eq!(
            ArktsExtendSource::new(source, ArktsLanguageVersion::V1_1).as_str(),
            "  @Extend(Text) function __Text__fancy(color:string){\n    .fontColor(color)\n  }\n"
        );
        assert_eq!(
            ArktsExtendSource::new(source, ArktsLanguageVersion::V1_2).as_str(),
            source
        );
    }

    #[test]
    fn preserves_upstream_global_text_matching_and_collects_modern_names() {
        let source = concat!(
            "const text = '@Extend Text.fake()';\n",
            "// @Extend Text.comment()\n",
            "@Extend(Text) function modern() {}\n"
        );
        let processed = ArktsExtendSource::new(source, ArktsLanguageVersion::V1_1);
        assert_eq!(
            processed.as_str(),
            concat!(
                "const text = '@Extend(Text) function __Text__fake()';\n",
                "// @Extend(Text) function __Text__comment()\n",
                "@Extend(Text) function modern() {}\n"
            )
        );
        assert_eq!(
            processed.legacy_names(),
            &BTreeSet::from(["comment".into(), "fake".into()])
        );
        assert_eq!(
            processed.modern_names(),
            &BTreeSet::from([
                "__Text__comment".into(),
                "__Text__fake".into(),
                "modern".into(),
            ])
        );
    }
}
