use oxc_allocator::Allocator;
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_parser::Parser;
use oxc_span::SourceType;

pub(crate) struct HvigorStandardTs<'a> {
    source: &'a str,
}

impl<'a> HvigorStandardTs<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        Self { source }
    }

    pub(crate) fn format(&self) -> String {
        let allocator = Allocator::default();
        let parsed =
            Parser::new(&allocator, self.source, SourceType::ets().with_module(true)).parse();
        if !parsed.diagnostics.is_empty() {
            return self.source.to_owned();
        }
        Codegen::new()
            .with_options(CodegenOptions::default())
            .build(&parsed.program)
            .code
    }
}
