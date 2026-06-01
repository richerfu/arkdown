use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, AssignmentTarget, BindingPattern, Declaration, Expression,
    Program, Statement,
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub struct ComponentMetadata {
    atomic_components: HashSet<String>,
    need_pop_components: HashSet<String>,
    enum_names: HashSet<String>,
}

impl ComponentMetadata {
    pub fn from_ets_loader_path(ets_loader_path: &Path) -> anyhow::Result<Self> {
        let components_dir = ets_loader_path.join("components");
        let mut atomic_components = HashSet::new();
        for entry in fs::read_dir(&components_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let value: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
            let Some(name) = value.get("name").and_then(Value::as_str) else {
                continue;
            };
            if value.get("atomic").and_then(Value::as_bool) == Some(true) {
                atomic_components.insert(name.to_string());
            }
        }

        let need_pop_components =
            load_need_pop_components(&ets_loader_path.join("lib/component_map.js"))?;
        let enum_names = load_sdk_enum_names(ets_loader_path)?;

        Ok(Self {
            atomic_components,
            need_pop_components,
            enum_names,
        })
    }

    pub fn needs_pop(&self, name: &str) -> bool {
        !self.atomic_components.contains(name) || self.need_pop_components.contains(name)
    }

    pub fn is_known_enum(&self, name: &str) -> bool {
        self.enum_names.contains(name)
    }
}

fn load_sdk_enum_names(ets_loader_path: &Path) -> anyhow::Result<HashSet<String>> {
    let roots = [
        ets_loader_path.join("declarations"),
        ets_loader_path.join("../../component"),
    ];
    let mut paths = Vec::new();
    for root in roots {
        collect_declaration_files(&root, &mut paths)?;
    }
    paths.sort();

    let mut names = HashSet::new();
    for path in paths {
        let source = fs::read_to_string(&path)?;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &source, SourceType::ts()).parse();
        if !parsed.errors.is_empty() {
            anyhow::bail!(
                "failed to parse SDK enum declarations at {}",
                path.display()
            );
        }
        names.extend(enum_names_from_program(&parsed.program));
    }
    Ok(names)
}

fn collect_declaration_files(root: &Path, paths: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_declaration_files(&path, paths)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("ts")
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".d.ts"))
        {
            paths.push(path);
        }
    }
    Ok(())
}

fn enum_names_from_program(program: &Program<'_>) -> HashSet<String> {
    let mut names = HashSet::new();
    for statement in &program.body {
        match statement {
            Statement::TSEnumDeclaration(declaration) => {
                names.insert(declaration.id.name.to_string());
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::TSEnumDeclaration(declaration)) = &export.declaration {
                    names.insert(declaration.id.name.to_string());
                }
            }
            _ => {}
        }
    }
    names
}

fn load_need_pop_components(component_map_path: &Path) -> anyhow::Result<HashSet<String>> {
    let source = fs::read_to_string(component_map_path)?;
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &source, SourceType::cjs()).parse();
    if !parsed.errors.is_empty() {
        anyhow::bail!(
            "failed to parse SDK component_map.js at {}",
            component_map_path.display()
        );
    }
    extract_need_pop_components(&parsed.program).ok_or_else(|| {
        anyhow::anyhow!(
            "failed to find NEEDPOP_COMPONENT in SDK component_map.js at {}",
            component_map_path.display()
        )
    })
}

fn extract_need_pop_components(program: &Program<'_>) -> Option<HashSet<String>> {
    for statement in &program.body {
        let Statement::VariableDeclaration(declaration) = statement else {
            continue;
        };
        for declarator in &declaration.declarations {
            let Some(init) = declarator.init.as_ref() else {
                continue;
            };
            if binding_name(&declarator.id) == Some("NEEDPOP_COMPONENT") {
                if let Some(values) = set_literal_strings(init) {
                    return Some(values);
                }
            }
            if let Some(values) = extract_need_pop_components_from_expression(init) {
                return Some(values);
            }
        }
    }
    None
}

fn extract_need_pop_components_from_expression(expr: &Expression<'_>) -> Option<HashSet<String>> {
    match expr {
        Expression::AssignmentExpression(assignment) => {
            if assignment_target_name(&assignment.left).as_deref() == Some("NEEDPOP_COMPONENT") {
                return set_literal_strings(&assignment.right);
            }
            extract_need_pop_components_from_expression(&assignment.right)
        }
        Expression::ParenthesizedExpression(parenthesized) => {
            extract_need_pop_components_from_expression(&parenthesized.expression)
        }
        Expression::SequenceExpression(sequence) => sequence
            .expressions
            .iter()
            .find_map(extract_need_pop_components_from_expression),
        _ => None,
    }
}

fn binding_name<'a>(pattern: &'a BindingPattern<'a>) -> Option<&'a str> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.as_str()),
        _ => None,
    }
}

fn assignment_target_name(target: &AssignmentTarget<'_>) -> Option<String> {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            Some(identifier.name.to_string())
        }
        _ => None,
    }
}

fn set_literal_strings(expr: &Expression<'_>) -> Option<HashSet<String>> {
    if let Expression::ParenthesizedExpression(parenthesized) = expr {
        return set_literal_strings(&parenthesized.expression);
    }
    let Expression::SequenceExpression(sequence) = expr else {
        return set_literal_strings_from_new_set(expr);
    };
    sequence
        .expressions
        .last()
        .and_then(set_literal_strings_from_new_set)
}

fn set_literal_strings_from_new_set(expr: &Expression<'_>) -> Option<HashSet<String>> {
    let Expression::NewExpression(new_expr) = expr else {
        return None;
    };
    let Expression::Identifier(callee) = &new_expr.callee else {
        return None;
    };
    if callee.name != "Set" {
        return None;
    }
    let Some(Argument::ArrayExpression(array)) = new_expr.arguments.first() else {
        return None;
    };
    let mut out = HashSet::new();
    for element in &array.elements {
        let ArrayExpressionElement::StringLiteral(literal) = element else {
            return None;
        };
        out.insert(literal.value.to_string());
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;

    fn parse_need_pop(source: &str) -> HashSet<String> {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::cjs()).parse();
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        extract_need_pop_components(&parsed.program).expect("NEEDPOP_COMPONENT")
    }

    #[test]
    fn extracts_need_pop_from_upstream_minified_declarator_shape() {
        let values = parse_need_pop(
            r#"const JS_BIND_COMPONENTS=new Set(["ForEach"]),NEEDPOP_COMPONENT=(exports.JS_BIND_COMPONENTS=JS_BIND_COMPONENTS,new Set(["Blank","Search"]));"#,
        );
        assert!(values.contains("Blank"));
        assert!(values.contains("Search"));
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn extracts_need_pop_from_assignment_initializer_shape() {
        let values = parse_need_pop(
            r#"const componentMap=(NEEDPOP_COMPONENT=new Set(["Blank","Search"]));"#,
        );
        assert!(values.contains("Blank"));
        assert!(values.contains("Search"));
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn loads_sdk_enum_names_from_declarations() {
        let metadata = ComponentMetadata::from_ets_loader_path(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ets-loader"
        )))
        .expect("test ets-loader metadata");
        assert!(metadata.is_known_enum("FontWeight"));
        assert!(metadata.is_known_enum("ImageInterpolation"));
    }
}
