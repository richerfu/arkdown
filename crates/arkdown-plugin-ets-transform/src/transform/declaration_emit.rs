use std::borrow::Cow;
use std::fmt::Write;

use oxc_ast::ast::{AnnotationDeclaration, AnnotationElement, FunctionBody};
use oxc_span::GetSpan;

use crate::transform::source_text::{span_text, type_annotation_text};

pub(crate) struct ParamsInterfaceEmitter<'a> {
    annotation: &'a AnnotationDeclaration<'a>,
    source_text: &'a str,
}

impl<'a> ParamsInterfaceEmitter<'a> {
    pub(crate) fn new(annotation: &'a AnnotationDeclaration<'a>, source_text: &'a str) -> Self {
        Self {
            annotation,
            source_text,
        }
    }

    pub(crate) fn emit(&self) -> String {
        let mut out = String::new();
        writeln!(out, "interface {} {{", self.annotation.id.name)
            .expect("writing to String cannot fail");
        for element in &self.annotation.body.body {
            let AnnotationElement::PropertyDefinition(property) = element;
            let Some(name) = property.key.static_name() else {
                continue;
            };
            let ty = property
                .type_annotation
                .as_ref()
                .map(|annotation| type_annotation_text(annotation.span(), self.source_text))
                .unwrap_or_else(|| "unknown".to_string());
            let optional = if property.optional { "?" } else { "" };
            writeln!(out, "    {name}{optional}: {ty};").expect("writing to String cannot fail");
        }
        out.push_str("}\n");
        out
    }

    pub(crate) fn emit_exported(&self) -> String {
        let mut out = String::from("export ");
        out.push_str(&self.emit());
        out
    }
}

pub(crate) struct BuilderParams;

impl BuilderParams {
    pub(crate) fn with_parent(params: String) -> String {
        if params.trim().is_empty() {
            "parent = null".to_string()
        } else {
            format!("{params}, parent = null")
        }
    }
}

pub(crate) struct TopLevelFunctionEmitter<'a> {
    exported: bool,
    name: Cow<'a, str>,
    params: &'a str,
    return_annotation: Cow<'a, str>,
    body: &'a str,
}

impl<'a> TopLevelFunctionEmitter<'a> {
    pub(crate) fn decorated(
        exported: bool,
        name: &'a str,
        params: &'a str,
        return_annotation: &'a str,
        body: &'a str,
    ) -> Self {
        Self {
            exported,
            name: Cow::Borrowed(name),
            params,
            return_annotation: Cow::Borrowed(return_annotation),
            body,
        }
    }

    pub(crate) fn extend(
        exported: bool,
        component: &str,
        name: &str,
        params: &'a str,
        body: &'a str,
    ) -> Self {
        Self {
            exported,
            name: Cow::Owned(format!("__{component}__{name}")),
            params,
            return_annotation: Cow::Borrowed(": void"),
            body,
        }
    }

    pub(crate) fn emit(&self) -> String {
        let mut out = String::new();
        if self.exported {
            out.push_str("export ");
        }
        write!(
            out,
            "function {}({}){} {{\n{}    }}",
            self.name, self.params, self.return_annotation, self.body
        )
        .expect("writing to String cannot fail");
        out
    }
}

pub(crate) struct ExtendBodyEmitter<'a> {
    body: Option<&'a FunctionBody<'a>>,
    source_text: &'a str,
    component: &'a str,
}

impl<'a> ExtendBodyEmitter<'a> {
    pub(crate) fn new(
        body: Option<&'a FunctionBody<'a>>,
        source_text: &'a str,
        component: &'a str,
    ) -> Self {
        Self {
            body,
            source_text,
            component,
        }
    }

    pub(crate) fn emit(&self) -> String {
        let Some(body) = self.body else {
            return String::new();
        };
        let body_text = span_text(body.span(), self.source_text);
        let inner = strip_outer_block(&body_text);
        let mut out = String::new();
        for line in inner.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            out.push_str("        ");
            if trimmed.starts_with('.') {
                out.push_str(self.component);
                out.push_str(trimmed);
            } else {
                out.push_str(trimmed);
            }
            if !trimmed.ends_with(';') && !trimmed.ends_with('{') && !trimmed.ends_with('}') {
                out.push(';');
            }
            out.push('\n');
        }
        out
    }
}

fn strip_outer_block(text: &str) -> &str {
    let trimmed = text.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].trim()
    } else {
        trimmed
    }
}
