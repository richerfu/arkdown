use oxc_ast::ast::ModuleExportName;
use string_wizard::MagicString;

#[derive(Debug)]
pub(crate) struct SourcePatch {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) replacement: String,
}

#[derive(Debug)]
pub(crate) struct AppliedSourcePatches {
    pub(crate) code: String,
    pub(crate) output_starts: Vec<Option<usize>>,
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

pub(crate) fn apply_source_patches(source: &str, patches: Vec<SourcePatch>) -> String {
    apply_source_patches_with_offsets(source, patches).code
}

pub(crate) fn apply_source_patches_with_offsets(
    source: &str,
    patches: Vec<SourcePatch>,
) -> AppliedSourcePatches {
    let patch_count = patches.len();
    if patches.is_empty() {
        return AppliedSourcePatches {
            code: source.to_string(),
            output_starts: Vec::new(),
        };
    }

    let mut indexed_patches = patches.into_iter().enumerate().collect::<Vec<_>>();
    indexed_patches.sort_by_key(|(_, patch)| patch.start);

    let mut magic = MagicString::new(source);
    let mut output_starts = vec![None; patch_count];
    let mut last_end = 0usize;
    let mut output_delta = 0isize;
    for (index, patch) in indexed_patches {
        if patch.start < last_end {
            continue;
        }

        output_starts[index] = Some((patch.start as isize + output_delta) as usize);
        let replaced_len = patch.end.saturating_sub(patch.start);
        let replacement_len = patch.replacement.len();
        output_delta += replacement_len as isize - replaced_len as isize;

        if patch.start == patch.end {
            magic.append_left(patch.start as u32, patch.replacement);
        } else {
            magic
                .update(patch.start as u32, patch.end as u32, patch.replacement)
                .expect("source patches must describe valid non-overlapping byte ranges");
        }
        last_end = patch.end;
    }

    AppliedSourcePatches {
        code: magic.to_string(),
        output_starts,
    }
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

    #[test]
    fn reports_replacement_output_starts() {
        let output = apply_source_patches_with_offsets(
            "abcdef",
            vec![
                SourcePatch {
                    start: 1,
                    end: 3,
                    replacement: "XXXX".to_string(),
                },
                SourcePatch {
                    start: 4,
                    end: 6,
                    replacement: "Y".to_string(),
                },
            ],
        );

        assert_eq!(output.code, "aXXXXdY");
        assert_eq!(output.output_starts, vec![Some(1), Some(6)]);
    }

    #[test]
    fn skipped_overlapping_patch_has_no_output_start() {
        let output = apply_source_patches_with_offsets(
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

        assert_eq!(output.code, "aXef");
        assert_eq!(output.output_starts, vec![Some(1), None]);
    }
}
