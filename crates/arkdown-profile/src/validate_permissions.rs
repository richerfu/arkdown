//! Request permission validation for `module.json5`.
//!
//! Mirrors the stable parts of hvigor `AbstractPreBuild.checkRequestPermissions()`:
//!
//! - duplicate `requestPermissions[].name` values are rejected;
//! - `user_grant` permissions must carry the mandatory user-facing fields;
//! - custom `definePermissions` can add or override `user_grant` membership;
//! - when the SDK previewer permission oracle is available, request names must
//!   be SDK-defined or declared under `definePermissions`.

use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};

use arkdown_core::ArkdownError;

const USER_GRANT_PERMISSIONS: &[&str] = &[
    "ohos.permission.ANSWER_CALL",
    "ohos.permission.ACCESS_BLUETOOTH",
    "ohos.permission.READ_CALENDAR",
    "ohos.permission.READ_CALL_LOG",
    "ohos.permission.READ_CELL_MESSAGES",
    "ohos.permission.READ_CONTACTS",
    "ohos.permission.READ_MESSAGES",
    "ohos.permission.RECEIVE_MMS",
    "ohos.permission.RECEIVE_SMS",
    "ohos.permission.RECEIVE_WAP_MESSAGES",
    "ohos.permission.MICROPHONE",
    "ohos.permission.SEND_MESSAGES",
    "ohos.permission.WRITE_CALENDAR",
    "ohos.permission.WRITE_CALL_LOG",
    "ohos.permission.WRITE_CONTACTS",
    "ohos.permission.DISTRIBUTED_DATASYNC",
    "ohos.permission.MANAGE_VOICEMAIL",
    "ohos.permission.LOCATION_IN_BACKGROUND",
    "ohos.permission.LOCATION",
    "ohos.permission.APPROXIMATELY_LOCATION",
    "ohos.permission.MEDIA_LOCATION",
    "ohos.permission.CAMERA",
    "ohos.permission.READ_MEDIA",
    "ohos.permission.WRITE_MEDIA",
    "ohos.permission.ACTIVITY_MOTION",
    "ohos.permission.READ_HEALTH_DATA",
    "ohos.permission.READ_IMAGEVIDEO",
    "ohos.permission.READ_AUDIO",
    "ohos.permission.READ_DOCUMENT",
    "ohos.permission.WRITE_IMAGEVIDEO",
    "ohos.permission.WRITE_AUDIO",
    "ohos.permission.WRITE_DOCUMENT",
    "ohos.permission.READ_WHOLE_CALENDAR",
    "ohos.permission.WRITE_WHOLE_CALENDAR",
    "ohos.permission.APP_TRACKING_CONSENT",
    "ohos.permission.GET_INSTALLED_BUNDLE_LIST",
    "ohos.permission.READ_PASTEBOARD",
    "ohos.permission.READ_WRITE_DOWNLOAD_DIRECTORY",
    "ohos.permission.READ_WRITE_DOCUMENTS_DIRECTORY",
    "ohos.permission.READ_WRITE_DESKTOP_DIRECTORY",
    "ohos.permission.ACCESS_NEARLINK",
];

pub struct RequestPermissionValidationInput<'a> {
    /// `module.type` from `module.json5`.
    pub module_type: &'a str,
    /// Source path used in diagnostics.
    pub module_json5_path: Option<&'a Path>,
    /// `module.requestPermissions`.
    pub request_permissions: &'a [serde_json::Value],
    /// `module.definePermissions`.
    pub define_permissions: &'a [serde_json::Value],
    /// SDK previewer permission oracle. When provided, unknown requested
    /// permission names are rejected unless they are declared under
    /// `definePermissions`.
    pub sdk_permissions: Option<&'a SdkPermissionData>,
}

/// Permission data extracted from SDK `previewer/common/resources/module.json`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SdkPermissionData {
    pub all_permissions: Vec<String>,
    pub user_grant_permissions: Vec<String>,
}

pub fn validate_request_permissions(
    input: RequestPermissionValidationInput<'_>,
) -> Result<(), ArkdownError> {
    if input.request_permissions.is_empty() {
        return Ok(());
    }

    let profile_path = display_profile_path(input.module_json5_path);
    let all_permissions = collect_all_permissions(input.define_permissions, input.sdk_permissions);
    let user_grant_permissions =
        collect_user_grant_permissions(input.define_permissions, input.sdk_permissions);
    let is_hap = is_hap_module_type(input.module_type);

    for permission in input.request_permissions {
        let Some(name) = permission_name(permission) else {
            continue;
        };

        if let Some(all_permissions) = &all_permissions {
            if !all_permissions.contains(name) {
                return Err(ArkdownError::Bundler(format!(
                    "The {name} permission under requestPermissions must be a value that is \
                     predefined within the SDK or a custom one that you have included under \
                     definePermissions.\nat {profile_path}"
                )));
            }
        }

        if user_grant_permissions.contains(name) {
            if is_hap && (!has_field(permission, "reason") || !has_field(permission, "usedScene")) {
                return Err(ArkdownError::Bundler(format!(
                    "The reason and usedScene attributes are mandatory for user_grant \
                     permissions. \n at {profile_path}"
                )));
            }
            if !is_hap && !has_field(permission, "reason") {
                return Err(ArkdownError::Bundler(format!(
                    "The reason attribute are mandatory for user_grant permissions. \n at \
                     {profile_path}"
                )));
            }
            // Validate usedScene.when value if present.
            // hvigor accepts "inuse" and "always"; any other value is an error.
            if let Some(used_scene) = permission.get("usedScene") {
                if let Some(when) = used_scene.get("when").and_then(|w| w.as_str()) {
                    if !matches!(when, "inuse" | "always") {
                        return Err(ArkdownError::Bundler(format!(
                            "The usedScene.when value '{}' is invalid for permission '{}'; \
                             expected 'inuse' or 'always'.\nat {profile_path}",
                            when, name
                        )));
                    }
                }
            }
        }
    }

    let duplicated = duplicated_permission_names(input.request_permissions);
    if !duplicated.is_empty() {
        return Err(ArkdownError::Bundler(format!(
            "Duplicate permission names detected under \"requestPermissions\": \"{}\". At file: \
             {profile_path}",
            duplicated.join("\",\"")
        )));
    }

    Ok(())
}

/// Parse SDK `previewer/common/resources/module.json#definePermissions`.
///
/// Missing files or empty/missing `definePermissions` return `Ok(None)`,
/// matching hvigor's `allowSdkPermission=false` fallback to the built-in
/// user-grant list without all-permission hard errors.
pub fn collect_sdk_permission_data(
    module_json_path: &Path,
) -> Result<Option<SdkPermissionData>, ArkdownError> {
    if !module_json_path.is_file() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(module_json_path)?;
    let value =
        json5::from_str::<serde_json::Value>(&content).map_err(|err| ArkdownError::ParseError {
            file: module_json_path.display().to_string(),
            message: err.to_string(),
        })?;
    let Some(define_permissions) = value
        .get("module")
        .and_then(|module| module.get("definePermissions"))
        .and_then(|permissions| permissions.as_array())
    else {
        return Ok(None);
    };
    if define_permissions.is_empty() {
        return Ok(None);
    }

    let mut all_seen = FxHashSet::default();
    let mut all_permissions = Vec::new();
    let mut user_grant_seen = FxHashSet::default();
    let mut user_grant_permissions = Vec::new();
    for permission in define_permissions {
        let Some(name) = permission_name(permission) else {
            continue;
        };
        if all_seen.insert(name.to_string()) {
            all_permissions.push(name.to_string());
        }
        if permission.get("grantMode").and_then(|value| value.as_str()) == Some("user_grant")
            && user_grant_seen.insert(name.to_string())
        {
            user_grant_permissions.push(name.to_string());
        }
    }

    Ok(Some(SdkPermissionData {
        all_permissions,
        user_grant_permissions,
    }))
}

/// Parse SDK `permissions.d.ts` files and return declared `ohos.permission.*`
/// names. Missing files are ignored so callers can decide whether SDK-name
/// validation is possible for the current SDK layout.
pub fn collect_sdk_permission_names(api_dirs: &[PathBuf]) -> Result<Vec<String>, ArkdownError> {
    let mut seen = FxHashSet::default();
    let mut names = Vec::new();

    for api_dir in api_dirs {
        let path = api_dir.join("permissions.d.ts");
        if !path.is_file() {
            continue;
        }
        let content = std::fs::read_to_string(&path)?;
        for (idx, segment) in content.split('\'').enumerate() {
            if idx % 2 == 0 || !segment.starts_with("ohos.permission.") {
                continue;
            }
            if seen.insert(segment.to_string()) {
                names.push(segment.to_string());
            }
        }
    }

    Ok(names)
}

fn collect_all_permissions(
    define_permissions: &[serde_json::Value],
    sdk_permissions: Option<&SdkPermissionData>,
) -> Option<FxHashSet<String>> {
    let sdk_permissions = sdk_permissions?;
    let mut all_permissions = sdk_permissions
        .all_permissions
        .iter()
        .cloned()
        .collect::<FxHashSet<_>>();
    let mut seen_define_permissions = FxHashSet::default();
    for permission in define_permissions {
        if let Some(name) = permission_name(permission) {
            if seen_define_permissions.insert(name.to_string()) {
                all_permissions.insert(name.to_string());
            }
        }
    }
    Some(all_permissions)
}

fn collect_user_grant_permissions(
    define_permissions: &[serde_json::Value],
    sdk_permissions: Option<&SdkPermissionData>,
) -> FxHashSet<String> {
    let mut user_grant = if let Some(sdk_permissions) = sdk_permissions {
        sdk_permissions
            .user_grant_permissions
            .iter()
            .cloned()
            .collect::<FxHashSet<_>>()
    } else {
        USER_GRANT_PERMISSIONS
            .iter()
            .map(|permission| (*permission).to_string())
            .collect::<FxHashSet<_>>()
    };
    let mut seen_define_permissions = FxHashSet::default();

    for permission in define_permissions {
        let Some(name) = permission_name(permission) else {
            continue;
        };
        if !seen_define_permissions.insert(name.to_string()) {
            continue;
        }
        if permission.get("grantMode").and_then(|value| value.as_str()) == Some("user_grant") {
            user_grant.insert(name.to_string());
        } else {
            user_grant.remove(name);
        }
    }

    user_grant
}

fn duplicated_permission_names(request_permissions: &[serde_json::Value]) -> Vec<String> {
    let mut seen = FxHashSet::default();
    let mut duplicated_seen = FxHashSet::default();
    let mut duplicated = Vec::new();

    for permission in request_permissions {
        let Some(name) = permission_name(permission) else {
            continue;
        };
        if !seen.insert(name.to_string()) && duplicated_seen.insert(name.to_string()) {
            duplicated.push(name.to_string());
        }
    }

    duplicated
}

fn permission_name(permission: &serde_json::Value) -> Option<&str> {
    permission.get("name").and_then(|value| value.as_str())
}

fn has_field(permission: &serde_json::Value, field: &str) -> bool {
    permission.get(field).is_some()
}

fn is_hap_module_type(module_type: &str) -> bool {
    !matches!(module_type, "shared" | "har")
}

fn display_profile_path(path: Option<&Path>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_else(|| "module.json5".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hap_user_grant_requires_reason_and_used_scene() {
        let err = validate_request_permissions(RequestPermissionValidationInput {
            module_type: "entry",
            module_json5_path: Some(Path::new("entry/src/main/module.json5")),
            request_permissions: &[serde_json::json!({
                "name": "ohos.permission.CAMERA",
                "reason": "$string:camera_reason",
            })],
            define_permissions: &[],
            sdk_permissions: None,
        })
        .unwrap_err();

        assert!(err.to_string().contains("reason and usedScene"));
    }

    #[test]
    fn test_hsp_user_grant_requires_reason_only() {
        let err = validate_request_permissions(RequestPermissionValidationInput {
            module_type: "shared",
            module_json5_path: Some(Path::new("shared/src/main/module.json5")),
            request_permissions: &[serde_json::json!({
                "name": "ohos.permission.CAMERA",
            })],
            define_permissions: &[],
            sdk_permissions: None,
        })
        .unwrap_err();
        assert!(err.to_string().contains("reason attribute"));

        validate_request_permissions(RequestPermissionValidationInput {
            module_type: "shared",
            module_json5_path: Some(Path::new("shared/src/main/module.json5")),
            request_permissions: &[serde_json::json!({
                "name": "ohos.permission.CAMERA",
                "reason": "$string:camera_reason",
            })],
            define_permissions: &[],
            sdk_permissions: None,
        })
        .unwrap();
    }

    #[test]
    fn test_define_permissions_update_user_grant_membership() {
        let custom_err = validate_request_permissions(RequestPermissionValidationInput {
            module_type: "entry",
            module_json5_path: None,
            request_permissions: &[serde_json::json!({
                "name": "com.example.permission.CUSTOM",
                "reason": "$string:custom_reason",
            })],
            define_permissions: &[serde_json::json!({
                "name": "com.example.permission.CUSTOM",
                "grantMode": "user_grant",
            })],
            sdk_permissions: None,
        })
        .unwrap_err();
        assert!(custom_err.to_string().contains("reason and usedScene"));

        validate_request_permissions(RequestPermissionValidationInput {
            module_type: "entry",
            module_json5_path: None,
            request_permissions: &[serde_json::json!({
                "name": "ohos.permission.CAMERA",
            })],
            define_permissions: &[serde_json::json!({
                "name": "ohos.permission.CAMERA",
                "grantMode": "system_grant",
            })],
            sdk_permissions: None,
        })
        .unwrap();
    }

    #[test]
    fn test_sdk_permission_data_reject_unknown_requests() {
        let sdk_permissions = SdkPermissionData {
            all_permissions: vec!["ohos.permission.CAMERA".to_string()],
            user_grant_permissions: vec!["ohos.permission.CAMERA".to_string()],
        };
        let err = validate_request_permissions(RequestPermissionValidationInput {
            module_type: "entry",
            module_json5_path: None,
            request_permissions: &[serde_json::json!({
                "name": "com.example.permission.CUSTOM",
            })],
            define_permissions: &[],
            sdk_permissions: Some(&sdk_permissions),
        })
        .unwrap_err();
        assert!(err.to_string().contains("predefined within the SDK"));

        validate_request_permissions(RequestPermissionValidationInput {
            module_type: "entry",
            module_json5_path: None,
            request_permissions: &[serde_json::json!({
                "name": "com.example.permission.CUSTOM",
            })],
            define_permissions: &[serde_json::json!({
                "name": "com.example.permission.CUSTOM",
                "grantMode": "system_grant",
            })],
            sdk_permissions: Some(&sdk_permissions),
        })
        .unwrap();
    }

    #[test]
    fn test_sdk_permission_user_grant_oracle_overrides_builtin_list() {
        let sdk_permissions = SdkPermissionData {
            all_permissions: vec![
                "ohos.permission.CAMERA".to_string(),
                "ohos.permission.SYSTEM_ONLY".to_string(),
            ],
            user_grant_permissions: vec![],
        };

        validate_request_permissions(RequestPermissionValidationInput {
            module_type: "entry",
            module_json5_path: None,
            request_permissions: &[serde_json::json!({
                "name": "ohos.permission.CAMERA",
            })],
            define_permissions: &[],
            sdk_permissions: Some(&sdk_permissions),
        })
        .unwrap();

        let err = validate_request_permissions(RequestPermissionValidationInput {
            module_type: "entry",
            module_json5_path: None,
            request_permissions: &[serde_json::json!({
                "name": "ohos.permission.SYSTEM_ONLY",
            })],
            define_permissions: &[serde_json::json!({
                "name": "ohos.permission.SYSTEM_ONLY",
                "grantMode": "user_grant",
            })],
            sdk_permissions: Some(&sdk_permissions),
        })
        .unwrap_err();
        assert!(err.to_string().contains("reason and usedScene"));
    }

    #[test]
    fn test_duplicate_request_permissions_are_rejected() {
        let err = validate_request_permissions(RequestPermissionValidationInput {
            module_type: "entry",
            module_json5_path: Some(Path::new("entry/src/main/module.json5")),
            request_permissions: &[
                serde_json::json!({
                    "name": "ohos.permission.ACCESS_BLUETOOTH",
                    "reason": "$string:bt_reason",
                    "usedScene": {},
                }),
                serde_json::json!({
                    "name": "ohos.permission.ACCESS_BLUETOOTH",
                    "reason": "$string:bt_reason",
                    "usedScene": {},
                }),
            ],
            define_permissions: &[],
            sdk_permissions: None,
        })
        .unwrap_err();

        assert!(err.to_string().contains("Duplicate permission names"));
    }

    #[test]
    fn test_collect_sdk_permission_names_parses_permissions_d_ts() {
        let tmp = tempfile::tempdir().unwrap();
        let api_dir = tmp.path().join("ets/api");
        std::fs::create_dir_all(&api_dir).unwrap();
        std::fs::write(
            api_dir.join("permissions.d.ts"),
            r#"
                export type Permissions =
                  'ohos.permission.CAMERA'
                  | 'ohos.permission.LOCATION'
                  | 'not.a.permission'
                  | 'ohos.permission.CAMERA'
            "#,
        )
        .unwrap();

        let names = collect_sdk_permission_names(&[api_dir]).unwrap();
        assert_eq!(
            names,
            vec![
                "ohos.permission.CAMERA".to_string(),
                "ohos.permission.LOCATION".to_string()
            ]
        );
    }

    /// Verify that `usedScene.when` only accepts `"inuse"` and `"always"`.
    /// Matches hvigor's PermissionUtils validation for user_grant permissions.
    #[test]
    fn test_hap_user_grant_invalid_used_scene_when_is_error() {
        let sdk = SdkPermissionData {
            all_permissions: vec!["ohos.permission.CAMERA".to_string()],
            user_grant_permissions: vec!["ohos.permission.CAMERA".to_string()],
        };
        // Invalid "when" value should fail.
        let err = validate_request_permissions(RequestPermissionValidationInput {
            module_type: "entry",
            module_json5_path: None,
            request_permissions: &[serde_json::json!({
                "name": "ohos.permission.CAMERA",
                "reason": "$string:camera_reason",
                "usedScene": { "when": "never" }  // invalid
            })],
            define_permissions: &[],
            sdk_permissions: Some(&sdk),
        })
        .unwrap_err();
        assert!(
            err.to_string().contains("never"),
            "error must mention the invalid value; got: {}",
            err
        );
        assert!(err.to_string().contains("inuse") || err.to_string().contains("always"));

        // Valid "when" values must pass.
        for valid_when in &["inuse", "always"] {
            let result = validate_request_permissions(RequestPermissionValidationInput {
                module_type: "entry",
                module_json5_path: None,
                request_permissions: &[serde_json::json!({
                    "name": "ohos.permission.CAMERA",
                    "reason": "$string:camera_reason",
                    "usedScene": { "when": valid_when }
                })],
                define_permissions: &[],
                sdk_permissions: Some(&sdk),
            });
            assert!(
                result.is_ok(),
                "usedScene.when={valid_when} must be valid; got: {:?}",
                result.err()
            );
        }
    }

    #[test]
    fn test_collect_sdk_permission_data_reads_previewer_module_json() {
        let tmp = tempfile::tempdir().unwrap();
        let module_json = tmp.path().join("previewer/common/resources/module.json");
        std::fs::create_dir_all(module_json.parent().unwrap()).unwrap();
        std::fs::write(
            &module_json,
            r#"{
                module: {
                    definePermissions: [
                        { name: "ohos.permission.CAMERA", grantMode: "user_grant" },
                        { name: "ohos.permission.SYSTEM_ONLY", grantMode: "system_grant" },
                        { name: "ohos.permission.CAMERA", grantMode: "user_grant" },
                    ],
                },
            }"#,
        )
        .unwrap();

        let data = collect_sdk_permission_data(&module_json).unwrap().unwrap();
        assert_eq!(
            data.all_permissions,
            vec![
                "ohos.permission.CAMERA".to_string(),
                "ohos.permission.SYSTEM_ONLY".to_string()
            ]
        );
        assert_eq!(
            data.user_grant_permissions,
            vec!["ohos.permission.CAMERA".to_string()]
        );
    }
}
