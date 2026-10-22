//! SDK system-resource table loading.
//!
//! Upstream `ets-loader/main.js` initializes `resources.sys` from
//! `ets-loader/sysResource.js` and merges optional external API resource tables.
//! This module reads the same JavaScript assignment through OXC AST and exposes
//! the resource references consumed by the `$r(...)` lowering pass.

use crate::text::StaticPropertyNameText;
use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    AssignmentTarget, Expression, ObjectExpression, ObjectPropertyKind, Program, Statement,
};
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::resource_transform::{
    ResourceReference, ResourceRuntimeKey, ResourceRuntimeType, ResourceSource,
};

pub struct SystemResourceReferenceLoader<'a> {
    ets_loader_path: &'a Path,
    external_api_paths: &'a [PathBuf],
    bundle_name: &'a str,
    module_name: &'a str,
}

impl<'a> SystemResourceReferenceLoader<'a> {
    pub fn new(
        ets_loader_path: &'a Path,
        external_api_paths: &'a [PathBuf],
        bundle_name: &'a str,
        module_name: &'a str,
    ) -> Self {
        Self {
            ets_loader_path,
            external_api_paths,
            bundle_name,
            module_name,
        }
    }

    pub fn load(&self) -> Result<FxHashMap<String, ResourceReference>> {
        let mut refs = FxHashMap::default();

        for path in
            SystemResourcePathInventory::new(self.ets_loader_path, self.external_api_paths).paths()
        {
            if !path.exists() {
                continue;
            }
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("read SDK system resources from {}", path.display()))?;
            refs.extend(
                SystemResourceSourceParser::new(&source, self.bundle_name, self.module_name)
                    .parse(),
            );
        }

        Ok(refs)
    }
}

struct SystemResourcePathInventory<'a> {
    ets_loader_path: &'a Path,
    external_api_paths: &'a [PathBuf],
}

impl<'a> SystemResourcePathInventory<'a> {
    fn new(ets_loader_path: &'a Path, external_api_paths: &'a [PathBuf]) -> Self {
        Self {
            ets_loader_path,
            external_api_paths,
        }
    }

    fn paths(&self) -> Vec<PathBuf> {
        let mut paths = vec![self.ets_loader_path.join("sysResource.js")];
        paths.extend(
            self.external_api_paths
                .iter()
                .map(|path| path.join("sysResource.js")),
        );
        paths
    }
}

struct SystemResourceSourceParser<'a> {
    source: &'a str,
    bundle_name: &'a str,
    module_name: &'a str,
}

impl<'a> SystemResourceSourceParser<'a> {
    fn new(source: &'a str, bundle_name: &'a str, module_name: &'a str) -> Self {
        Self {
            source,
            bundle_name,
            module_name,
        }
    }

    fn parse(&self) -> FxHashMap<String, ResourceReference> {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, self.source, SourceType::default()).parse();
        let Some(sys_object) = SystemResourceObjectFinder::new(&parsed.program).find() else {
            return FxHashMap::default();
        };

        SystemResourceCollector::new(sys_object, self.bundle_name, self.module_name).collect()
    }
}

struct SystemResourceObjectFinder<'a> {
    program: &'a Program<'a>,
}

impl<'a> SystemResourceObjectFinder<'a> {
    fn new(program: &'a Program<'a>) -> Self {
        Self { program }
    }

    fn find(&self) -> Option<&'a ObjectExpression<'a>> {
        self.program.body.iter().find_map(|statement| {
            let Statement::ExpressionStatement(statement) = statement else {
                return None;
            };
            let Expression::AssignmentExpression(assignment) = &statement.expression else {
                return None;
            };
            if !Self::is_module_exports_sys_target(&assignment.left) {
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
        if sys_access.property.name != SystemResourceExportName::SYS {
            return false;
        }

        let Expression::StaticMemberExpression(exports_access) = &sys_access.object else {
            return false;
        };
        if exports_access.property.name != SystemResourceExportName::EXPORTS {
            return false;
        }

        matches!(
            &exports_access.object,
            Expression::Identifier(identifier) if identifier.name == SystemResourceExportName::MODULE
        )
    }
}

struct SystemResourceExportName;

impl SystemResourceExportName {
    const MODULE: &'static str = "module";
    const EXPORTS: &'static str = "exports";
    const SYS: &'static str = "sys";
}

struct SystemResourceCollector<'a> {
    sys_object: &'a ObjectExpression<'a>,
    bundle_name: &'a str,
    module_name: &'a str,
}

impl<'a> SystemResourceCollector<'a> {
    fn new(
        sys_object: &'a ObjectExpression<'a>,
        bundle_name: &'a str,
        module_name: &'a str,
    ) -> Self {
        Self {
            sys_object,
            bundle_name,
            module_name,
        }
    }

    fn collect(&self) -> FxHashMap<String, ResourceReference> {
        let mut refs = FxHashMap::default();
        for type_property in &self.sys_object.properties {
            let ObjectPropertyKind::ObjectProperty(type_property) = type_property else {
                continue;
            };
            let Some(resource_type) = type_property.key.static_name() else {
                continue;
            };
            let Some(runtime_type) =
                ResourceRuntimeType::from_name(resource_type.as_str_or_empty())
            else {
                continue;
            };
            let Expression::ObjectExpression(entries) = &type_property.value else {
                continue;
            };

            for resource_property in &entries.properties {
                let ObjectPropertyKind::ObjectProperty(resource_property) = resource_property
                else {
                    continue;
                };
                let Some(resource_name) = resource_property.key.static_name() else {
                    continue;
                };
                let Some(id) = SystemResourceId::new(&resource_property.value).numeric() else {
                    continue;
                };
                refs.insert(
                    ResourceRuntimeKey::new(
                        ResourceSource::Sys,
                        resource_type.as_str_or_empty(),
                        resource_name.as_str_or_empty(),
                    )
                    .to_key(),
                    ResourceReference {
                        id,
                        runtime_type: runtime_type.value(),
                        bundle_name: self.bundle_name.to_string(),
                        module_name: self.module_name.to_string(),
                    },
                );
            }
        }
        refs
    }
}

struct SystemResourceId<'a> {
    expression: &'a Expression<'a>,
}

impl<'a> SystemResourceId<'a> {
    fn new(expression: &'a Expression<'a>) -> Self {
        Self { expression }
    }

    fn numeric(&self) -> Option<u32> {
        let Expression::NumericLiteral(literal) = self.expression else {
            return None;
        };
        let value = literal.value;
        (value.is_finite() && value.fract() == 0.0 && value >= 0.0 && value <= f64::from(u32::MAX))
            .then_some(value as u32)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{SystemResourceReferenceLoader, SystemResourceSourceParser};

    #[test]
    fn parses_sdk_sys_resource_assignment() {
        let refs = SystemResourceSourceParser::new(
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
        )
        .parse();

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

    #[test]
    fn loads_external_sdk_resources_from_explicit_snapshot_paths() {
        let temp = tempfile::tempdir().expect("temporary SDK");
        let loader = temp.path().join("openharmony/ets/build-tools/ets-loader");
        let external = temp.path().join("vendor/ets");
        fs::create_dir_all(&loader).expect("primary loader");
        fs::create_dir_all(&external).expect("external ETS component");
        fs::write(
            loader.join("sysResource.js"),
            "module.exports.sys = { color: { primary: 1, shared: 2 } };",
        )
        .expect("primary resources");
        fs::write(
            external.join("sysResource.js"),
            "module.exports.sys = { color: { shared: 3, external: 4 } };",
        )
        .expect("external resources");

        let refs = SystemResourceReferenceLoader::new(
            &loader,
            std::slice::from_ref(&external),
            "com.example.test",
            "entry",
        )
        .load()
        .expect("system resources");

        assert_eq!(refs["sys.color.primary"].id, 1);
        assert_eq!(refs["sys.color.shared"].id, 3);
        assert_eq!(refs["sys.color.external"].id, 4);
    }
}
