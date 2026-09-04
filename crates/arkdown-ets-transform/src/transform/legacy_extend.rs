use std::borrow::Cow;

use arkdown_core::ArktsLanguageVersion;
use oxc_syntax::identifier::{is_identifier_part, is_identifier_start};

/// Parser input adapter for ArkTS 1.1's original `@Extend Component.name()`
/// declaration spelling.
///
/// The SDK parser represents this spelling as the same decorated function AST
/// produced by `@Extend(Component) function name()`. OXC intentionally exposes
/// the modern AST shape, so normalize only declaration-leading source lines
/// before parsing and keep the rest of the transform entirely AST based.
pub(crate) struct LegacyExtendSource<'a> {
    source: Cow<'a, str>,
}

impl<'a> LegacyExtendSource<'a> {
    pub(crate) fn new(source: &'a str, version: ArktsLanguageVersion) -> Self {
        if version.is_static()
            || (!source.contains("@Extend ") && !source.contains("@AnimatableExtend "))
        {
            return Self {
                source: Cow::Borrowed(source),
            };
        }

        let mut normalized = String::with_capacity(source.len());
        let mut changed = false;
        for line in source.split_inclusive('\n') {
            if let Some(replacement) = Self::normalized_line(line) {
                normalized.push_str(&replacement);
                changed = true;
            } else {
                normalized.push_str(line);
            }
        }
        if changed {
            Self {
                source: Cow::Owned(normalized),
            }
        } else {
            Self {
                source: Cow::Borrowed(source),
            }
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        self.source.as_ref()
    }

    fn normalized_line(line: &str) -> Option<String> {
        let source = line.trim_start_matches([' ', '\t']);
        let indentation = &line[..line.len() - source.len()];
        let (decorator, tail) = ["@Extend", "@AnimatableExtend"]
            .into_iter()
            .find_map(|decorator| source.strip_prefix(decorator).map(|tail| (decorator, tail)))?;
        if !tail.starts_with([' ', '\t']) {
            return None;
        }

        let tail = tail.trim_start_matches([' ', '\t']);
        let (component, tail) = Self::identifier(tail)?;
        let tail = tail.trim_start_matches([' ', '\t']);
        let tail = tail.strip_prefix('.')?.trim_start_matches([' ', '\t']);
        let (name, tail) = Self::identifier(tail)?;
        if !tail.trim_start_matches([' ', '\t']).starts_with('(') {
            return None;
        }

        Some(format!(
            "{indentation}{decorator}({component}) function {name}{tail}"
        ))
    }

    fn identifier(source: &str) -> Option<(&str, &str)> {
        let mut chars = source.char_indices();
        let (_, first) = chars.next()?;
        if !is_identifier_start(first) {
            return None;
        }
        let mut end = first.len_utf8();
        for (offset, character) in chars {
            if !is_identifier_part(character) {
                break;
            }
            end = offset + character.len_utf8();
        }
        Some((&source[..end], &source[end..]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_original_extend_declarations_only_in_arkts_11() {
        let source = "  @Extend Text.fancy(color:string){\n    .fontColor(color)\n  }\n";
        assert_eq!(
            LegacyExtendSource::new(source, ArktsLanguageVersion::V1_1).as_str(),
            "  @Extend(Text) function fancy(color:string){\n    .fontColor(color)\n  }\n"
        );
        assert_eq!(
            LegacyExtendSource::new(source, ArktsLanguageVersion::V1_2).as_str(),
            source
        );
    }

    #[test]
    fn ignores_strings_comments_and_modern_extend_declarations() {
        let source = concat!(
            "const text = '@Extend Text.fake()';\n",
            "// @Extend Text.comment()\n",
            "@Extend(Text) function modern() {}\n"
        );
        assert_eq!(
            LegacyExtendSource::new(source, ArktsLanguageVersion::V1_1).as_str(),
            source
        );
    }
}
