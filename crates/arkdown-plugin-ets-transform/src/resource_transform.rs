//! Resource-call lowering aligned with SDK `ets-loader` `processResourceData`.
//!
//! The matcher operates on OXC AST call expressions, mirroring upstream's
//! TypeScript AST visitor: only `$r(...)` and `$rawfile(...)` call nodes are
//! rewritten.  The replacement text is emitted from a small structured model so
//! the field order and static names stay centralized instead of being assembled
//! ad hoc at call sites.

use std::collections::HashMap;
use std::fmt::Write;

use oxc_allocator::Allocator;
use oxc_ast::ast::{CallExpression, Expression};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

use crate::{apply_source_patches, SourcePatch};

const RESOURCE_CALL: &str = "$r";
const RAWFILE_CALL: &str = "$rawfile";
pub(crate) const RAWFILE_RUNTIME_TYPE: i64 = 30000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceReference {
    pub id: u32,
    pub runtime_type: u32,
    pub bundle_name: String,
    pub module_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResourceObject {
    id: i64,
    runtime_type: i64,
    params: Vec<String>,
    bundle_name: String,
    module_name: String,
}

impl ResourceObject {
    fn app_static(reference: &ResourceReference, params: Vec<String>) -> Self {
        Self {
            id: i64::from(reference.id),
            runtime_type: i64::from(reference.runtime_type),
            params,
            bundle_name: reference.bundle_name.clone(),
            module_name: reference.module_name.clone(),
        }
    }

    fn app_dynamic(params: Vec<String>, names: &ResourceNames) -> Self {
        Self {
            id: -1,
            runtime_type: -1,
            params,
            bundle_name: names.bundle_name.clone(),
            module_name: names.module_name.clone(),
        }
    }

    fn rawfile(id: i64, params: Vec<String>, names: &ResourceNames) -> Self {
        Self {
            id,
            runtime_type: RAWFILE_RUNTIME_TYPE,
            params,
            bundle_name: names.bundle_name.clone(),
            module_name: names.module_name.clone(),
        }
    }

    fn emit(&self) -> String {
        let mut out = String::new();
        out.push_str("{ \"id\": ");
        let _ = write!(out, "{}", self.id);
        out.push_str(", \"type\": ");
        let _ = write!(out, "{}", self.runtime_type);
        out.push_str(", params: [");
        for (index, param) in self.params.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push_str(param);
        }
        out.push_str("], \"bundleName\": ");
        push_quoted_js_string(&mut out, &self.bundle_name);
        out.push_str(", \"moduleName\": ");
        push_quoted_js_string(&mut out, &self.module_name);
        out.push_str(" }");
        out
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ResourceNames {
    bundle_name: String,
    module_name: String,
}

impl ResourceNames {
    fn from_refs(resource_refs: &HashMap<String, ResourceReference>) -> Self {
        resource_refs
            .values()
            .next()
            .map(|reference| Self {
                bundle_name: reference.bundle_name.clone(),
                module_name: reference.module_name.clone(),
            })
            .unwrap_or_default()
    }
}

pub(crate) fn rewrite_resource_references(
    source: &str,
    id: &str,
    resource_refs: &HashMap<String, ResourceReference>,
) -> String {
    let allocator = Allocator::default();
    let source_type =
        SourceType::from_path(id).unwrap_or_else(|_| SourceType::ts().with_module(true));
    let parsed = Parser::new(&allocator, source, source_type).parse();

    let mut visitor = ResourceReferenceVisitor {
        source,
        resource_refs,
        names: ResourceNames::from_refs(resource_refs),
        patches: Vec::new(),
    };
    visitor.visit_program(&parsed.program);
    apply_source_patches(source, visitor.patches)
}

struct ResourceReferenceVisitor<'a> {
    source: &'a str,
    resource_refs: &'a HashMap<String, ResourceReference>,
    names: ResourceNames,
    patches: Vec<SourcePatch>,
}

impl<'a> Visit<'a> for ResourceReferenceVisitor<'_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Some(replacement) = self.resource_replacement(call) {
            self.patches.push(SourcePatch {
                start: call.span.start as usize,
                end: call.span.end as usize,
                replacement,
            });
            return;
        }
        walk::walk_call_expression(self, call);
    }
}

impl ResourceReferenceVisitor<'_> {
    fn resource_replacement(&self, call: &CallExpression<'_>) -> Option<String> {
        let Expression::Identifier(callee) = &call.callee else {
            return None;
        };

        let replacement = match callee.name.as_str() {
            RESOURCE_CALL => self.resource_object(call),
            RAWFILE_CALL => self.rawfile_object(call),
            _ => return None,
        }?;
        Some(replacement.emit())
    }

    fn resource_object(&self, call: &CallExpression<'_>) -> Option<ResourceObject> {
        let first_arg = call.arguments.first()?.as_expression()?;
        if let Expression::StringLiteral(resource_name) = first_arg {
            let resource_key = resource_key(resource_name.value.as_str())?;
            let resource = self.resource_refs.get(&resource_key)?;
            return Some(ResourceObject::app_static(
                resource,
                collect_argument_sources(self.source, call, 1),
            ));
        }

        Some(ResourceObject::app_dynamic(
            collect_argument_sources(self.source, call, 0),
            &self.names,
        ))
    }

    fn rawfile_object(&self, call: &CallExpression<'_>) -> Option<ResourceObject> {
        let first_arg = call.arguments.first()?;
        let is_static_string = first_arg
            .as_expression()
            .is_some_and(|expr| matches!(expr, Expression::StringLiteral(_)));
        let params = if is_static_string {
            vec![self.source[first_arg.span()].trim().to_string()]
        } else {
            collect_argument_sources(self.source, call, 0)
        };
        Some(ResourceObject::rawfile(
            if is_static_string { 0 } else { -1 },
            params,
            &self.names,
        ))
    }
}

fn collect_argument_sources(source: &str, call: &CallExpression<'_>, skip: usize) -> Vec<String> {
    call.arguments
        .iter()
        .skip(skip)
        .map(|argument| source[argument.span()].trim().to_string())
        .collect()
}

pub(crate) fn resource_runtime_type(resource_type: &str) -> Option<u32> {
    match resource_type {
        "color" => Some(10001),
        "float" => Some(10002),
        "string" => Some(10003),
        "plural" => Some(10004),
        "boolean" => Some(10005),
        "intarray" => Some(10006),
        "integer" => Some(10007),
        "pattern" => Some(10008),
        "strarray" => Some(10009),
        "media" => Some(20000),
        "rawfile" => Some(RAWFILE_RUNTIME_TYPE as u32),
        "symbol" => Some(40000),
        _ => None,
    }
}

fn resource_key(resource_name: &str) -> Option<String> {
    let (source, name) = resource_name
        .split_once('.')
        .filter(|(source, _)| matches!(*source, "app" | "sys"))?;
    let mut parts = name.splitn(3, '.');
    let ty = parts.next()?;
    let resource = parts.next()?;
    (parts.next().is_none()
        && !ty.is_empty()
        && !resource.is_empty()
        && resource_runtime_type(ty).is_some())
    .then(|| format!("{source}.{name}"))
}

fn push_quoted_js_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('"');
}
