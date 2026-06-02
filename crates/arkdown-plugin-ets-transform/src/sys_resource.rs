//! SDK system-resource table loading.
//!
//! Upstream `ets-loader/main.js` initializes `resources.sys` from
//! `ets-loader/sysResource.js` and merges optional external API resource tables.
//! This module reads the same JavaScript assignment through OXC AST and exposes
//! the resource references consumed by the `$r(...)` lowering pass.

use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    AssignmentTarget, Expression, ObjectExpression, ObjectPropertyKind, Program, Statement,
};
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::resource_transform::{resource_runtime_type, ResourceReference};

pub fn load_system_resource_references(
    ets_loader_path: &Path,
    bundle_name: &str,
    module_name: &str,
) -> Result<FxHashMap<String, ResourceReference>> {
    let mut refs = FxHashMap::default();

    for path in system_resource_paths(ets_loader_path) {
        if !path.exists() {
            continue;
        }
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("read SDK system resources from {}", path.display()))?;
        refs.extend(parse_system_resource_source(
            &source,
            bundle_name,
            module_name,
        ));
    }

    Ok(refs)
}

fn system_resource_paths(ets_loader_path: &Path) -> Vec<PathBuf> {
    let mut paths = vec![ets_loader_path.join("sysResource.js")];

    if let Some(external_api_paths) = std::env::var_os("externalApiPaths") {
        paths.extend(
            std::env::split_paths(&external_api_paths)
                .map(|path| ets_loader_path.join(path).join("sysResource.js")),
        );
    }

    paths
}

fn parse_system_resource_source(
    source: &str,
    bundle_name: &str,
    module_name: &str,
) -> FxHashMap<String, ResourceReference> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::default()).parse();
    let Some(sys_object) = find_system_resource_object(&parsed.program) else {
        return FxHashMap::default();
    };

    let mut refs = FxHashMap::default();
    collect_system_resources(sys_object, bundle_name, module_name, &mut refs);
    refs
}

fn find_system_resource_object<'a>(program: &'a Program<'a>) -> Option<&'a ObjectExpression<'a>> {
    program.body.iter().find_map(|statement| {
        let Statement::ExpressionStatement(statement) = statement else {
            return None;
        };
        let Expression::AssignmentExpression(assignment) = &statement.expression else {
            return None;
        };
        if !is_module_exports_sys_target(&assignment.left) {
            return None;
        }
        let Expression::ObjectExpression(object) = &assignment.right else {
            return None;
        };
        Some(object.as_ref())
    })
}

fn is_module_exports_sys_target(target: &AssignmentTarget<'_>) -> bool {
    let AssignmentTarget::StaticMemberExpression(sys_access) = target else {
        return false;
    };
    if sys_access.property.name != "sys" {
        return false;
    }

    let Expression::StaticMemberExpression(exports_access) = &sys_access.object else {
        return false;
    };
    if exports_access.property.name != "exports" {
        return false;
    }

    matches!(
        &exports_access.object,
        Expression::Identifier(identifier) if identifier.name == "module"
    )
}

fn collect_system_resources(
    sys_object: &ObjectExpression<'_>,
    bundle_name: &str,
    module_name: &str,
    refs: &mut FxHashMap<String, ResourceReference>,
) {
    for type_property in &sys_object.properties {
        let ObjectPropertyKind::ObjectProperty(type_property) = type_property else {
            continue;
        };
        let Some(resource_type) = type_property.key.static_name() else {
            continue;
        };
        let Some(runtime_type) = resource_runtime_type(resource_type.as_ref()) else {
            continue;
        };
        let Expression::ObjectExpression(entries) = &type_property.value else {
            continue;
        };

        for resource_property in &entries.properties {
            let ObjectPropertyKind::ObjectProperty(resource_property) = resource_property else {
                continue;
            };
            let Some(resource_name) = resource_property.key.static_name() else {
                continue;
            };
            let Some(id) = numeric_resource_id(&resource_property.value) else {
                continue;
            };
            refs.insert(
                format!("sys.{}.{}", resource_type, resource_name),
                ResourceReference {
                    id,
                    runtime_type,
                    bundle_name: bundle_name.to_string(),
                    module_name: module_name.to_string(),
                },
            );
        }
    }
}

fn numeric_resource_id(expression: &Expression<'_>) -> Option<u32> {
    let Expression::NumericLiteral(literal) = expression else {
        return None;
    };
    let value = literal.value;
    (value.is_finite() && value.fract() == 0.0 && value >= 0.0 && value <= f64::from(u32::MAX))
        .then_some(value as u32)
}

#[cfg(test)]
mod tests {
    use super::parse_system_resource_source;

    #[test]
    fn parses_sdk_sys_resource_assignment() {
        let refs = parse_system_resource_source(
            r#"
module.exports.sys = {
  color: {
    ohos_id_color_emphasize: 125829135,
  },
  symbol: {
    chevron_right: 125836000,
  },
  unknown: {
    skipped: 1,
  },
}
"#,
            "com.example.test",
            "entry",
        );

        assert_eq!(refs["sys.color.ohos_id_color_emphasize"].id, 125829135);
        assert_eq!(
            refs["sys.color.ohos_id_color_emphasize"].runtime_type,
            10001
        );
        assert_eq!(refs["sys.symbol.chevron_right"].runtime_type, 40000);
        assert!(!refs.contains_key("sys.unknown.skipped"));
        assert_eq!(
            refs["sys.symbol.chevron_right"].bundle_name,
            "com.example.test"
        );
        assert_eq!(refs["sys.symbol.chevron_right"].module_name, "entry");
    }
}
