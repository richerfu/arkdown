//! Minification pass using `oxc_minifier`.
//!
//! Applied after OHOS identifier renaming when the `-compact` rule is active.
//! Eliminates dead code, folds constants, merges consecutive declarations, and
//! strips whitespace/comments via the oxc codegen minify flag.

use oxc_allocator::Allocator;
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_minifier::{CompressOptions, MangleOptions, Minifier, MinifierOptions};
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;

use crate::ObfuscationError;

/// Minify `source`, returning the minified JS string.
///
/// Applies dead-code elimination, constant folding, variable joining, and
/// whitespace removal.  Identifier mangling runs on top of any prior OHOS
/// identifier renaming, further compressing remaining names.
pub fn minify_source(source: &str, filename: &str) -> Result<String, ObfuscationError> {
    let alloc = Allocator::default();
    let source_type = SourceType::default().with_module(true);
    let ParserReturn {
        mut program,
        errors,
        ..
    } = Parser::new(&alloc, source, source_type).parse();

    if !errors.is_empty() {
        return Err(ObfuscationError::ParseError {
            file: filename.to_string(),
            message: errors[0].message.to_string(),
        });
    }

    let options = MinifierOptions {
        compress: Some(CompressOptions {
            drop_console: false,
            ..CompressOptions::smallest()
        }),
        mangle: Some(MangleOptions::default()),
    };

    Minifier::new(options).minify(&alloc, &mut program);

    let codegen_out = Codegen::new()
        .with_options(CodegenOptions::minify())
        .build(&program);

    Ok(codegen_out.code)
}
