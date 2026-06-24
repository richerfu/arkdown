//! Source emit helpers for ArkTS transform lowering.
//!
//! Upstream ets-loader keeps original AST nodes as nodes and lets the TypeScript
//! printer preserve their source shape where possible. Centralize remaining
//! source text boundaries here while generated expression text is printed by OXC
//! codegen.

pub(crate) struct CodegenSourceText {
    code: String,
}

impl CodegenSourceText {
    pub(crate) fn new(code: String) -> Self {
        Self { code }
    }

    pub(crate) fn without_trailing_newline(mut self) -> String {
        if self.code.ends_with('\n') {
            self.code.pop();
        }
        self.code
    }
}
