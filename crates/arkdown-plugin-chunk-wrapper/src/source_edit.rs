#[derive(Debug)]
pub(crate) struct SourcePatch {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) replacement: String,
}

pub(crate) fn apply_source_patches(source: &str, mut patches: Vec<SourcePatch>) -> String {
    if patches.is_empty() {
        return source.to_owned();
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
