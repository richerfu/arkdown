use oxc_ast::ast::ModuleExportName;

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
    let mut out = String::with_capacity(source.len());
    let mut last_end = 0usize;
    for patch in patches {
        if patch.start < last_end {
            continue;
        }
        out.push_str(&source[last_end..patch.start]);
        out.push_str(&patch.replacement);
        last_end = patch.end;
    }
    out.push_str(&source[last_end..]);
    out
}
