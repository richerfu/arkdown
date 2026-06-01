use oxc_ast::ast::ModuleExportName;
use string_wizard::MagicString;

#[derive(Debug)]
pub(crate) struct SourcePatch {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) replacement: String,
}

pub(crate) fn module_export_name_text(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(identifier) => identifier.name.to_string(),
        ModuleExportName::IdentifierReference(identifier) => identifier.name.to_string(),
        ModuleExportName::StringLiteral(literal) => literal.raw.as_ref().map_or_else(
            || format!("{:?}", literal.value.as_str()),
            ToString::to_string,
        ),
    }
}

pub(crate) fn apply_source_patches(source: &str, mut patches: Vec<SourcePatch>) -> String {
    if patches.is_empty() {
        return source.to_string();
    }

    patches.sort_by_key(|patch| patch.start);
    let mut magic = MagicString::new(source);
    let mut last_end = 0usize;
    for patch in patches {
        if patch.start < last_end {
            continue;
        }

        if patch.start == patch.end {
            magic.append_left(patch.start as u32, patch.replacement);
        } else {
            magic
                .update(patch.start as u32, patch.end as u32, patch.replacement)
                .expect("source patches must describe valid non-overlapping byte ranges");
        }
        last_end = patch.end;
    }
    magic.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_ordered_replacements_with_magic_string() {
        let output = apply_source_patches(
            "abcdef",
            vec![
                SourcePatch {
                    start: 1,
                    end: 3,
                    replacement: "XX".to_string(),
                },
                SourcePatch {
                    start: 4,
                    end: 6,
                    replacement: "YY".to_string(),
                },
            ],
        );

        assert_eq!(output, "aXXdYY");
    }

    #[test]
    fn applies_zero_length_insertions_before_index() {
        let output = apply_source_patches(
            "abcd",
            vec![SourcePatch {
                start: 2,
                end: 2,
                replacement: "XX".to_string(),
            }],
        );

        assert_eq!(output, "abXXcd");
    }

    #[test]
    fn skips_overlapping_patches_like_previous_manual_applier() {
        let output = apply_source_patches(
            "abcdef",
            vec![
                SourcePatch {
                    start: 1,
                    end: 4,
                    replacement: "X".to_string(),
                },
                SourcePatch {
                    start: 2,
                    end: 5,
                    replacement: "Y".to_string(),
                },
            ],
        );

        assert_eq!(output, "aXef");
    }
}
