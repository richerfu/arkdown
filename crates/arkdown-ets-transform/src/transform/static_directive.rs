use std::borrow::Cow;

use oxc_allocator::Allocator;
use oxc_ast::ast::{Directive, Program, StringLiteral};
use oxc_ast::builder::AstBuilder;
use oxc_span::Span;

/// Parser input for the ArkTS 1.2 program-level `use static` directive.
///
/// Current OpenHarmony ArkTS samples place the directive before imports. The
/// pinned ohos-rs/OXC static parser otherwise counts that directive as a
/// non-import top-level item and rejects every following import. Masking the
/// directive with same-length spaces preserves every downstream source span;
/// [`Self::restore`] reconstructs the source-owned directive AST before any
/// transformation or code generation runs.
pub(crate) struct StaticDirectiveSource<'a> {
    source: Cow<'a, str>,
    directive_span: Option<Span>,
    expression_span: Option<Span>,
}

impl<'a> StaticDirectiveSource<'a> {
    const DIRECTIVE: &'static str = "use static";

    pub(crate) fn new(source: &'a str) -> Self {
        let Some((directive_span, expression_span)) = Self::directive_spans(source) else {
            return Self {
                source: Cow::Borrowed(source),
                directive_span: None,
                expression_span: None,
            };
        };

        let mut masked = source.to_string();
        masked.replace_range(
            directive_span.start as usize..directive_span.end as usize,
            &" ".repeat((directive_span.end - directive_span.start) as usize),
        );
        Self {
            source: Cow::Owned(masked),
            directive_span: Some(directive_span),
            expression_span: Some(expression_span),
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        self.source.as_ref()
    }

    pub(crate) fn restore<'alloc>(
        &self,
        program: &mut Program<'alloc>,
        allocator: &'alloc Allocator,
    ) {
        let (Some(directive_span), Some(expression_span)) =
            (self.directive_span, self.expression_span)
        else {
            return;
        };
        let ast = AstBuilder::new(allocator);
        program.directives.insert(
            0,
            Directive::new(
                directive_span,
                StringLiteral::new(expression_span, Self::DIRECTIVE, None, &ast),
                Self::DIRECTIVE,
                &ast,
            ),
        );
    }

    fn directive_spans(source: &str) -> Option<(Span, Span)> {
        let bytes = source.as_bytes();
        let mut cursor = usize::from(bytes.starts_with(&[0xEF, 0xBB, 0xBF])) * 3;
        cursor = Self::skip_trivia(bytes, cursor)?;

        let quote = *bytes.get(cursor)?;
        if quote != b'\'' && quote != b'"' {
            return None;
        }
        let expression_start = cursor;
        cursor += 1;
        let directive_end = cursor.checked_add(Self::DIRECTIVE.len())?;
        if bytes.get(cursor..directive_end)? != Self::DIRECTIVE.as_bytes()
            || *bytes.get(directive_end)? != quote
        {
            return None;
        }
        cursor = directive_end + 1;
        let expression_end = cursor;
        while matches!(bytes.get(cursor), Some(b' ' | b'\t')) {
            cursor += 1;
        }
        if bytes.get(cursor) == Some(&b';') {
            cursor += 1;
        } else if !matches!(bytes.get(cursor), None | Some(b'\n' | b'\r')) {
            return None;
        }

        Some((
            Span::new(expression_start as u32, cursor as u32),
            Span::new(expression_start as u32, expression_end as u32),
        ))
    }

    fn skip_trivia(bytes: &[u8], mut cursor: usize) -> Option<usize> {
        loop {
            while matches!(
                bytes.get(cursor),
                Some(b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
            ) {
                cursor += 1;
            }
            match (bytes.get(cursor), bytes.get(cursor + 1)) {
                (Some(b'/'), Some(b'/')) => {
                    cursor += 2;
                    while !matches!(bytes.get(cursor), None | Some(b'\n' | b'\r')) {
                        cursor += 1;
                    }
                }
                (Some(b'/'), Some(b'*')) => {
                    cursor += 2;
                    while !matches!(
                        (bytes.get(cursor), bytes.get(cursor + 1)),
                        (Some(b'*'), Some(b'/'))
                    ) {
                        bytes.get(cursor)?;
                        cursor += 1;
                    }
                    cursor += 2;
                }
                _ => return Some(cursor),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    use super::*;

    #[test]
    fn masks_and_restores_upstream_static_sample_directive_before_imports() {
        let source = concat!(
            "'use static';\n",
            "/* OpenHarmony sample license header */\n",
            "import UIAbility from '@ohos.app.ability.UIAbility';\n",
            "class EntryAbility extends UIAbility {}\n"
        );
        let adapted = StaticDirectiveSource::new(source);
        assert_eq!(adapted.as_str().len(), source.len());

        let allocator = Allocator::default();
        let mut parsed =
            Parser::new(&allocator, adapted.as_str(), SourceType::ets_static()).parse();
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        adapted.restore(&mut parsed.program, &allocator);

        assert_eq!(parsed.program.directives.len(), 1);
        assert_eq!(parsed.program.directives[0].directive, "use static");
        assert_eq!(
            parsed.program.directives[0].span,
            Span::new(0, "'use static';".len() as u32)
        );
    }
}
