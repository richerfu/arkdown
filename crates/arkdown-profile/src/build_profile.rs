//! Module `BuildProfile.ets` generation.
//!
//! Upstream ownership:
//! - `hvigor-ohos-plugin/src/tasks/build-profile-task.ts`
//! - `hvigor-ohos-plugin/src/tasks/har/har-build-profile-task.ts`
//! - `hvigor-ohos-plugin/src/utils/build-profile-utils.ts`

use std::path::Path;

use arkdown_core::{ArkdownError, BuildMode};

const CONST_HEADER: &str = "/**\n * Use these variables when you tailor your ArkTS code. They must be of the const type.\n */\n";
const CLASS_HEADER: &str =
    "/**\n * BuildProfile Class is used only for compatibility purposes.\n */\nexport default class BuildProfile { \n";

/// Hvigor task variant that owns the generated profile.
pub enum BuildProfileKind<'a> {
    /// HAP/HSP application profile.
    Application {
        bundle_name: &'a str,
        bundle_type: &'a str,
        version_code: u64,
        version_name: &'a str,
        product_name: &'a str,
        build_version: Option<&'a str>,
    },
    /// Source/obfuscated/bytecode HAR profile.
    Har { version: &'a str },
}

/// Immutable inputs to one `BuildProfile.ets` generation.
pub struct BuildProfileInput<'a> {
    pub output_path: &'a Path,
    pub profile_path: &'a Path,
    pub kind: BuildProfileKind<'a>,
    pub target_name: &'a str,
    pub build_mode: &'a BuildMode,
    pub fields: &'a serde_json::Map<String, serde_json::Value>,
}

/// Writes the generated module profile in hvigor's observable field order.
pub struct BuildProfileWriter;

impl BuildProfileWriter {
    pub fn new() -> Self {
        Self
    }

    pub fn write(&self, input: BuildProfileInput<'_>) -> Result<(), ArkdownError> {
        let source = self.render(&input)?;
        if let Some(parent) = input.output_path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        fs_err::write(input.output_path, source)?;
        Ok(())
    }

    pub fn render(&self, input: &BuildProfileInput<'_>) -> Result<String, ArkdownError> {
        let mut values = serde_json::Map::new();
        match input.kind {
            BuildProfileKind::Application {
                bundle_name,
                bundle_type,
                version_code,
                version_name,
                product_name,
                build_version,
            } => {
                values.insert("BUNDLE_NAME".into(), bundle_name.into());
                values.insert("BUNDLE_TYPE".into(), bundle_type.into());
                values.insert("VERSION_CODE".into(), version_code.into());
                values.insert("VERSION_NAME".into(), version_name.into());
                values.insert("TARGET_NAME".into(), input.target_name.into());
                values.insert("PRODUCT_NAME".into(), product_name.into());
                values.insert("BUILD_MODE_NAME".into(), self.build_mode_name(input).into());
                values.insert("DEBUG".into(), self.debug(input).into());
                if let Some(build_version) = build_version {
                    values.insert("BUILD_VERSION".into(), build_version.into());
                }
            }
            BuildProfileKind::Har { version } => {
                values.insert("HAR_VERSION".into(), version.into());
                values.insert("BUILD_MODE_NAME".into(), self.build_mode_name(input).into());
                values.insert("DEBUG".into(), self.debug(input).into());
                values.insert("TARGET_NAME".into(), input.target_name.into());
            }
        }
        for (name, value) in input.fields {
            self.validate_field(input.profile_path, name, value)?;
            values.insert(name.clone(), value.clone());
        }

        let mut source = String::from(CONST_HEADER);
        for (name, value) in &values {
            source.push_str("export const ");
            source.push_str(name);
            source.push_str(" = ");
            self.push_value(&mut source, value);
            source.push_str(";\n");
        }
        source.push('\n');
        source.push_str(CLASS_HEADER);
        for name in values.keys() {
            source.push_str("\tstatic readonly ");
            source.push_str(name);
            source.push_str(" = ");
            source.push_str(name);
            source.push_str(";\n");
        }
        source.push('}');
        Ok(source)
    }

    fn build_mode_name(&self, input: &BuildProfileInput<'_>) -> &'static str {
        match input.build_mode {
            BuildMode::Debug => "debug",
            BuildMode::Release => "release",
        }
    }

    fn debug(&self, input: &BuildProfileInput<'_>) -> bool {
        matches!(input.build_mode, BuildMode::Debug)
    }

    fn validate_field(
        &self,
        profile_path: &Path,
        name: &str,
        value: &serde_json::Value,
    ) -> Result<(), ArkdownError> {
        if value.is_string() || value.is_number() || value.is_boolean() {
            return Ok(());
        }
        Err(ArkdownError::ParseError {
            file: profile_path.display().to_string(),
            message: format!(
                "buildOption.arkOptions.buildProfileFields.{name} only supports string, number, or boolean values; got {}",
                self.value_kind(value)
            ),
        })
    }

    fn value_kind(&self, value: &serde_json::Value) -> &'static str {
        match value {
            serde_json::Value::Null => "null",
            serde_json::Value::Bool(_) => "boolean",
            serde_json::Value::Number(_) => "number",
            serde_json::Value::String(_) => "string",
            serde_json::Value::Array(_) => "array",
            serde_json::Value::Object(_) => "object",
        }
    }

    fn push_value(&self, source: &mut String, value: &serde_json::Value) {
        match value {
            serde_json::Value::String(value) => {
                // This deliberately mirrors hvigor's `generateConstVariable`,
                // which emits single-quoted source rather than JSON strings.
                source.push('\'');
                source.push_str(value);
                source.push('\'');
            }
            serde_json::Value::Bool(value) => {
                source.push_str(if *value { "true" } else { "false" });
            }
            serde_json::Value::Number(value) => source.push_str(&value.to_string()),
            _ => unreachable!("validated by BuildProfileWriter::validate_field"),
        }
    }
}

impl Default for BuildProfileWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_profile_matches_hvigor_field_order_and_custom_override() {
        let mut fields = serde_json::Map::new();
        fields.insert("MTDEV".into(), true.into());
        fields.insert("VERSION_NAME".into(), "custom".into());
        let output = std::path::Path::new("BuildProfile.ets");
        let profile = std::path::Path::new("build-profile.json5");
        let source = BuildProfileWriter::new()
            .render(&BuildProfileInput {
                output_path: output,
                profile_path: profile,
                kind: BuildProfileKind::Application {
                    bundle_name: "com.example.app",
                    bundle_type: "app",
                    version_code: 100,
                    version_name: "1.0.0",
                    product_name: "default",
                    build_version: Some("2026.07.23"),
                },
                target_name: "default",
                build_mode: &BuildMode::Debug,
                fields: &fields,
            })
            .unwrap();

        assert!(source.contains("export const VERSION_NAME = 'custom';"));
        assert!(source.contains("export const MTDEV = true;"));
        assert!(
            source.find("VERSION_NAME").unwrap() < source.find("TARGET_NAME").unwrap(),
            "overriding an existing key must keep its original object-spread position"
        );
        assert!(
            source.find("DEBUG").unwrap() < source.find("BUILD_VERSION").unwrap(),
            "hvigor emits BUILD_VERSION immediately after DEBUG"
        );
        assert!(
            source.find("BUILD_VERSION").unwrap() < source.find("MTDEV").unwrap(),
            "new custom fields must follow built-in fields"
        );
    }

    #[test]
    fn unsupported_custom_value_is_rejected_with_profile_path() {
        let fields = serde_json::json!({"INVALID": {"nested": true}})
            .as_object()
            .unwrap()
            .clone();
        let error = BuildProfileWriter::new()
            .render(&BuildProfileInput {
                output_path: Path::new("BuildProfile.ets"),
                profile_path: Path::new("entry/build-profile.json5"),
                kind: BuildProfileKind::Har { version: "1.0.0" },
                target_name: "default",
                build_mode: &BuildMode::Release,
                fields: &fields,
            })
            .unwrap_err();

        assert!(error.to_string().contains("entry/build-profile.json5"));
        assert!(error.to_string().contains("only supports"));
    }
}
