//! `build-info` oracle parity tests.
//!
//! The expectations in this file were captured verbatim from hvigor 6.24.3
//! (`hvigorw buildInfo -p json= -p buildOption=` and `-p module=entry ...`)
//! running on the `minimal-hap` fixture, so a failure here is a real
//! divergence from hvigor's `BuildInfo` task, not a cosmetic difference.

use std::path::PathBuf;

use arkdown_napi::cmd::build_info::build_info_value;
use serde_json::Value;

fn fixture_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("..")
    .join("..")
    .join("tests")
    .join("fixtures")
    .join("minimal-hap")
}

#[test]
fn project_build_info_matches_hvigor_resolved_oracle() {
  let info = build_info_value(&fixture_root(), None, true).expect("build-info project value");
  assert_eq!(
    info["product"],
    serde_json::json!(["default"]),
    "product list from build-profile.json5#app.products"
  );
  assert_eq!(info["module"], serde_json::json!(["entry"]));
  assert_eq!(
    info["buildMode"],
    serde_json::json!(["debug", "release"]),
    "buildMode list from build-profile.json5#app.buildModeSet"
  );
  // Captured from `hvigorw buildInfo -p json= -p buildOption=`.
  let expected: Value = serde_json::from_str(
    r#"{
      "default": {
        "entry": {
          "default": {
            "debug": {
              "debuggable": true,
              "copyFrom": "default",
              "name": "debug",
              "strictMode": {
                "caseSensitiveCheck": false,
                "useNormalizedOHMUrl": false
              },
              "resOptions": {
                "copyCodeResource": { "enable": false }
              }
            },
            "release": {
              "debuggable": false,
              "copyFrom": "default",
              "name": "release",
              "strictMode": {
                "caseSensitiveCheck": false,
                "useNormalizedOHMUrl": false
              },
              "resOptions": {
                "copyCodeResource": { "enable": false }
              },
              "arkOptions": {
                "obfuscation": {
                  "ruleOptions": {
                    "enable": false,
                    "files": ["./obfuscation-rules.txt"]
                  }
                }
              }
            }
          }
        }
      }
    }"#,
  )
  .expect("hvigor oracle JSON");
  assert_eq!(
    info["buildInfo"], expected,
    "resolved buildInfo tree must match hvigor `-p buildOption=` byte-for-byte"
  );
}

#[test]
fn project_build_info_without_resolve_shows_empty_entries_like_hvigor() {
  let info = build_info_value(&fixture_root(), None, false).expect("build-info project value");
  assert_eq!(
    info["buildInfo"]["default"]["entry"]["default"]["debug"],
    serde_json::json!({}),
    "without `-p buildOption=` every entry is an empty object (hvigor `i = \"\" === e.buildOption`)"
  );
}

#[test]
fn module_build_info_reads_module_level_targets_like_hvigor() {
  let info = build_info_value(&fixture_root(), Some("entry@default"), true)
    .expect("build-info module value");
  // hvigor's module node enumerates `entry/build-profile.json5#targets`
  // (default + ohosTest), not the project-level subset.
  let targets = info["buildInfo"]
    .as_object()
    .expect("buildInfo object")
    .keys()
    .cloned()
    .collect::<Vec<_>>();
  assert_eq!(targets, vec!["default", "ohosTest"]);

  // The ohosTest target binds to the `default` option set via hvigor's
  // synthesized buildModeBinder: `name: "default"`, no copyFrom, and no
  // release obfuscation.
  let ohos_test_debug = &info["buildInfo"]["ohosTest"]["debug"];
  assert_eq!(ohos_test_debug["name"], "default");
  assert!(ohos_test_debug.get("copyFrom").is_none());
  assert_eq!(ohos_test_debug["debuggable"], true);
  let ohos_test_release = &info["buildInfo"]["ohosTest"]["release"];
  assert!(ohos_test_release.get("arkOptions").is_none());
}
