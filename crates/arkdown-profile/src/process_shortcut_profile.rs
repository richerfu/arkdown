//! Resource-phase shortcut profile customization.
//!
//! Mirrors the `CompileResource.customizeShortcut()` path in hvigor:
//! after restool copies profile resources, profiles referenced by
//! `module.metadata[].resource` that contain a `shortcuts` array are rewritten
//! to `{"newShortCutObj": <shortcut>}`. For wants targeting the app's original
//! bundle name, hvigor rewrites `bundleName` to the selected product bundle
//! name.

use std::path::Path;

use arkdown_core::{ArkdownError, MetadataEntry};

use crate::layout::{ProfileModuleLayout, ProfileOutputLayout};
use crate::resource_ref::ProfileResourceRef;

/// Input for hvigor's resource-phase shortcut profile customization.
pub struct ShortcutProfileInput<'a> {
    /// Root directory of the module, e.g. `/project/entry`.
    pub module_root: &'a Path,
    /// Module `metadata` array from `module.json5`.
    pub metadata: &'a [MetadataEntry],
    /// Bundle name from `AppScope/app.json5`.
    pub app_bundle_name: &'a str,
    /// Bundle name selected for the active product.
    pub product_bundle_name: &'a str,
    /// Resource profile output directory, normally
    /// `build/default/intermediates/res/default/resources/base/profile`.
    pub output_profile_dir: &'a Path,
}

/// Processes shortcut profile resources after resource compilation.
///
/// Mirrors the `CompileResource.customizeShortcut()` path in hvigor:
/// profiles referenced by `module.metadata[].resource` that contain a
/// `shortcuts` array are rewritten to `{"newShortCutObj": <shortcut>}`.
/// For wants targeting the app's original bundle name, `bundleName` is
/// rewritten to the selected product bundle name.
pub struct ShortcutProfileProcessor {
    /// Module name, used for diagnostics (currently reserved for future use).
    pub module_name: String,
}

impl ShortcutProfileProcessor {
    /// Create a new processor for the given module.
    pub fn new(module_name: impl Into<String>) -> Self {
        ShortcutProfileProcessor {
            module_name: module_name.into(),
        }
    }

    /// Rewrite shortcut profile resources after resource compilation.
    ///
    /// Returns the number of output profile files written. Invalid/missing
    /// source profiles are skipped, matching hvigor's best-effort JSON5 reads.
    pub fn process_shortcut_profiles(
        &self,
        input: ShortcutProfileInput<'_>,
    ) -> Result<usize, ArkdownError> {
        let mut written = 0;
        for metadata in input.metadata {
            let Some(resource) = metadata.resource.as_deref() else {
                continue;
            };
            let Some(profile_name) = parse_profile_name(resource) else {
                continue;
            };
            let output_profile =
                ProfileOutputLayout::new(input.output_profile_dir).profile_json(profile_name);
            let fallback_source_profile =
                ProfileModuleLayout::new(input.module_root).profile_json(profile_name);
            let Some(source_value) = read_json5_value(&output_profile)
                .or_else(|| read_json5_value(&fallback_source_profile))
            else {
                continue;
            };
            let Some(shortcuts) = source_value
                .get("shortcuts")
                .and_then(|value| value.as_array())
            else {
                continue;
            };
            if shortcuts.is_empty() {
                continue;
            }

            for shortcut in shortcuts {
                let Some(wants) = shortcut.get("wants").and_then(|value| value.as_array()) else {
                    continue;
                };
                if wants.is_empty() {
                    continue;
                }
                let mut shortcut = shortcut.clone();
                rewrite_shortcut_bundle_name(
                    &mut shortcut,
                    input.app_bundle_name,
                    input.product_bundle_name,
                );
                if let Some(parent) = output_profile.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(
                    &output_profile,
                    serde_json::to_vec(&serde_json::json!({ "newShortCutObj": shortcut }))
                        .expect("serializing shortcut profile should not fail"),
                )?;
                written += 1;
            }
        }
        Ok(written)
    }
}

impl Default for ShortcutProfileProcessor {
    fn default() -> Self {
        ShortcutProfileProcessor::new("")
    }
}

fn parse_profile_name(resource: &str) -> Option<&str> {
    ProfileResourceRef::parse(resource).map(|profile_ref| profile_ref.name())
}

fn read_json5_value(path: &Path) -> Option<serde_json::Value> {
    let content = std::fs::read_to_string(path).ok()?;
    json5::from_str(&content).ok()
}

fn rewrite_shortcut_bundle_name(
    shortcut: &mut serde_json::Value,
    app_bundle_name: &str,
    product_bundle_name: &str,
) {
    let Some(wants) = shortcut
        .get_mut("wants")
        .and_then(|value| value.as_array_mut())
    else {
        return;
    };
    for want in wants {
        let Some(object) = want.as_object_mut() else {
            continue;
        };
        if object
            .get("bundleName")
            .and_then(|value| value.as_str())
            .is_some_and(|bundle_name| bundle_name == app_bundle_name)
        {
            object.insert("bundleName".into(), product_bundle_name.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(resource: &str) -> MetadataEntry {
        MetadataEntry {
            name: "shortcut".to_string(),
            value: None,
            resource: Some(resource.to_string()),
        }
    }

    #[test]
    fn test_process_shortcut_profiles_rewrites_bundle_name() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_dir = tmp.path().join("src/main/resources/base/profile");
        std::fs::create_dir_all(&profile_dir).unwrap();
        std::fs::write(
            profile_dir.join("shortcut_config.json"),
            r#"{
                shortcuts: [
                    {
                        shortcutId: "first",
                        wants: [
                            { bundleName: "com.example.base", abilityName: "EntryAbility" },
                            { bundleName: "com.other.app", abilityName: "OtherAbility" },
                        ],
                    },
                    {
                        shortcutId: "second",
                        wants: [
                            { bundleName: "com.example.base", abilityName: "SecondAbility" },
                        ],
                    },
                ],
            }"#,
        )
        .unwrap();
        let output_profile_dir = tmp
            .path()
            .join("build/default/intermediates/res/default/resources/base/profile");

        let written = ShortcutProfileProcessor::new("entry")
            .process_shortcut_profiles(ShortcutProfileInput {
                module_root: tmp.path(),
                metadata: &[metadata("$profile:shortcut_config")],
                app_bundle_name: "com.example.base",
                product_bundle_name: "com.example.product",
                output_profile_dir: &output_profile_dir,
            })
            .unwrap();

        assert_eq!(written, 2);
        let value: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(output_profile_dir.join("shortcut_config.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(value["newShortCutObj"]["shortcutId"], "second");
        assert_eq!(
            value["newShortCutObj"]["wants"][0]["bundleName"],
            "com.example.product"
        );
    }

    #[test]
    fn test_process_shortcut_profiles_reads_compiled_output_profile_first() {
        let tmp = tempfile::tempdir().unwrap();
        let output_profile_dir = tmp
            .path()
            .join("build/default/intermediates/res/default/resources/base/profile");
        std::fs::create_dir_all(&output_profile_dir).unwrap();
        std::fs::write(
            output_profile_dir.join("shortcut_config.json"),
            r#"{
                shortcuts: [{
                    shortcutId: "target-resource",
                    wants: [{ bundleName: "com.example.base", abilityName: "EntryAbility" }],
                }],
            }"#,
        )
        .unwrap();

        let written = ShortcutProfileProcessor::new("entry")
            .process_shortcut_profiles(ShortcutProfileInput {
                module_root: tmp.path(),
                metadata: &[metadata("$profile:shortcut_config")],
                app_bundle_name: "com.example.base",
                product_bundle_name: "com.example.product",
                output_profile_dir: &output_profile_dir,
            })
            .unwrap();

        assert_eq!(written, 1);
        let value: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(output_profile_dir.join("shortcut_config.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(value["newShortCutObj"]["shortcutId"], "target-resource");
        assert_eq!(
            value["newShortCutObj"]["wants"][0]["bundleName"],
            "com.example.product"
        );
    }

    #[test]
    fn test_process_shortcut_profiles_ignores_profiles_without_wants() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_dir = tmp.path().join("src/main/resources/base/profile");
        std::fs::create_dir_all(&profile_dir).unwrap();
        std::fs::write(
            profile_dir.join("shortcut_config.json"),
            r#"{ shortcuts: [{ shortcutId: "empty" }] }"#,
        )
        .unwrap();
        let output_profile_dir = tmp
            .path()
            .join("build/default/intermediates/res/default/resources/base/profile");

        let written = ShortcutProfileProcessor::new("entry")
            .process_shortcut_profiles(ShortcutProfileInput {
                module_root: tmp.path(),
                metadata: &[metadata("$profile:shortcut_config")],
                app_bundle_name: "com.example.base",
                product_bundle_name: "com.example.product",
                output_profile_dir: &output_profile_dir,
            })
            .unwrap();

        assert_eq!(written, 0);
        assert!(!output_profile_dir.join("shortcut_config.json").exists());
    }

    #[test]
    fn test_shortcut_profile_processor_new_stores_module_name() {
        let processor = ShortcutProfileProcessor::new("entry");
        assert_eq!(processor.module_name, "entry");
    }

    #[test]
    fn test_shortcut_profile_processor_default_empty_module_name() {
        let processor = ShortcutProfileProcessor::default();
        assert_eq!(processor.module_name, "");
    }

    #[test]
    fn test_shortcut_profile_processor_rewrites_bundle_name() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_dir = tmp.path().join("src/main/resources/base/profile");
        std::fs::create_dir_all(&profile_dir).unwrap();
        std::fs::write(
            profile_dir.join("shortcut_config.json"),
            r#"{
                shortcuts: [{
                    shortcutId: "test",
                    wants: [{ bundleName: "com.example.base", abilityName: "EntryAbility" }],
                }],
            }"#,
        )
        .unwrap();
        let output_profile_dir = tmp
            .path()
            .join("build/default/intermediates/res/default/resources/base/profile");

        let processor = ShortcutProfileProcessor::new("entry");
        let written = processor
            .process_shortcut_profiles(ShortcutProfileInput {
                module_root: tmp.path(),
                metadata: &[metadata("$profile:shortcut_config")],
                app_bundle_name: "com.example.base",
                product_bundle_name: "com.example.product",
                output_profile_dir: &output_profile_dir,
            })
            .unwrap();

        assert_eq!(written, 1);
        let value: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(output_profile_dir.join("shortcut_config.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            value["newShortCutObj"]["wants"][0]["bundleName"],
            "com.example.product"
        );
    }
}
