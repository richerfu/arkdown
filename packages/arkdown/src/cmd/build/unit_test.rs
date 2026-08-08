//! Local unit-test chain (`hvigorw test` parity).
//!
//! Mirrors hvigor 6.24.3 `GenerateUnitTestTemplate`, `UnitTestProcessProfile`
//! and `ReplaceUnitTestIndexFile` for the local unit-test build root
//! (`buildRoot=.test`):
//!
//! * `generate_unit_test_template` writes the three `@ohos/coverage` templates
//!   into `<module>/.test/testability/` — `pages/Index.ets` (parameterized),
//!   `TestAbility.ets` (fixed) and `phoneSettingConfig_Phone.json` (fixed).
//!   The byte-for-byte content matches `@ohos/coverage/lib/src/commandLine/
//!   localTest/testTemplate.js` with `fileOperte.js` write semantics.
//! * `inject_unit_test_ability` appends the synthetic `TestAbility` entry to
//!   `intermediates/process_profile/<target>/module.json` (HAR/HSP type
//!   rewrites included) and overwrites `intermediates/res/<target>/
//!   ark_module.json` with the same document.
//! * `replace_unit_test_index_file` rewrites `intermediates/res/<target>/
//!   module.json` `pages` and emits `resources/base/profile/main_pages.json`
//!   from the `unit.test.replace.page` default
//!   (`../../../.test/testability/pages/Index`).

use std::path::Path;

use anyhow::Result;
use serde_json::Value;

/// `CommonConst.UNIT_TEST` — `testFrameworkPar.testMode` for local tests.
pub(crate) const UNIT_TEST_MODE: &str = "unitTest";
/// `CommonConst.TEST_ABILITY` — synthetic ability name.
const TEST_ABILITY: &str = "TestAbility";
/// `UnitTestConst.ENTRY_TEST_ABILITY_PATH` — srcEntrance of the synthetic ability.
const ENTRY_TEST_ABILITY_PATH: &str = "../../.test/testability/TestAbility.ets";
/// `UnitTestConst.TEST_ABILITY_ETS_PATH` — existence probe under the build root.
const TEST_ABILITY_ETS_PATH: &str = "testability/TestAbility.ets";
/// `CommonConst.ICON` / `CommonConst.LABEL` — HAR rewrites.
const ICON: &str = "icon";
const LABEL: &str = "label";
/// hvigor `defaultTestProps["unit.test.replace.page"]` (InjectConst.
/// UNIT_TEST_REPLACE_PAGE).
pub(crate) const UNIT_TEST_REPLACE_PAGE: &str = "../../../.test/testability/pages/Index";
/// `ReplaceUnitTestIndexFile` page resource id.
const PAGE_JSON_RESOURCE: &str = "$profile:main_pages";

/// Parameters for the local unit-test index page template
/// (`@ohos/coverage` `pageIndexTemplate`).
pub(crate) struct UnitTestTemplateParams<'a> {
  /// `app.json5` bundle name.
  pub bundle_name: &'a str,
  /// Module name (`-m` parameter).
  pub module_name: &'a str,
  /// hvigor extra config `scope` (`OHOS_TEST_COVERAGE_SCOPE`); empty when unset.
  pub scope: &'a str,
  /// `ohos-test-coverage` extra config: `true`/`false` string in the template.
  pub coverage: bool,
  /// Absolute module path (`pathInfo.getModulePath()`), `/`-normalized.
  pub module_path: &'a Path,
}

/// `@ohos/coverage` `pageIndexTemplate(testTemplateParam)` with
/// `fileOperte.js` path handling (`modulePath/.test/testability/pages/Index.ets`).
pub(crate) fn render_page_index_template(params: &UnitTestTemplateParams<'_>) -> String {
  let module_path = arkdown_path::ArtifactPathSerializer::path_to_slash_string(params.module_path)
    .unwrap_or_else(|_| params.module_path.display().to_string());
  let coverage = if params.coverage { "true" } else { "false" };
  let scope = params.scope;
  PAGE_INDEX_TEMPLATE
    .replace("__MODULE_PATH__", &module_path)
    .replace("__BUNDLE_NAME__", params.bundle_name)
    .replace("__MODULE_NAME__", params.module_name)
    .replace("__SCOPE__", scope)
    .replace("__COVERAGE__", coverage)
}

/// Write the three local unit-test template files into
/// `<module>/.test/testability/`. Mirrors `GenerateUnitTestTemplate`
/// calling `@ohos/coverage` `generateTemplateFile`.
pub(crate) fn generate_unit_test_template(
  module_root: &Path,
  params: &UnitTestTemplateParams<'_>,
) -> Result<()> {
  let testability = module_root.join(".test").join("testability");
  let pages = testability.join("pages");
  std::fs::create_dir_all(&pages)?;
  std::fs::write(pages.join("Index.ets"), render_page_index_template(params))?;
  std::fs::write(testability.join("TestAbility.ets"), TEST_ABILITY_TEMPLATE)?;
  std::fs::write(
    testability.join("phoneSettingConfig_Phone.json"),
    PHONE_SETTING_CONFIG_TEMPLATE,
  )?;
  Ok(())
}

/// Whether the generated testability template exists under the module build
/// root — `UnitTestProcessProfile` returns early (no TestAbility injection)
/// when `<buildRoot>/testability/TestAbility.ets` is missing.
pub(crate) fn has_test_ability_template(build_root: &Path) -> bool {
  build_root.join(TEST_ABILITY_ETS_PATH).is_file()
}

/// `UnitTestProcessProfile.doTaskAction` — append the synthetic `TestAbility`
/// to the processed module document and write both the processed profile
/// (`module.json`, `\t` indentation) and the ark module json.
///
/// `module_type` is `"entry"`/`"feature"`/`"shared"`/`"har"` as resolved by
/// the module model; HAR/HSP modules get `deliveryWithInstall` rewritten and
/// HAR additionally `module.type = "entry"` plus `app.icon`/`app.label`.
pub(crate) fn inject_unit_test_ability(
  process_profile: &mut Value,
  module_type: &str,
) -> Result<()> {
  let document = process_profile.as_object_mut().ok_or_else(|| {
    anyhow::anyhow!("process_profile module.json is not a JSON object")
  })?;

  let is_har = module_type == "har";
  let is_hsp = module_type == "shared";
  if is_hsp || is_har {
    let module = document
      .get_mut("module")
      .and_then(Value::as_object_mut)
      .ok_or_else(|| anyhow::anyhow!("process_profile module.json lacks module object"))?;
    module.insert("deliveryWithInstall".into(), Value::Bool(is_har));
    if is_har {
      module.insert("type".into(), "entry".into());
      let app = document
        .get_mut("app")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow::anyhow!("process_profile module.json lacks app object"))?;
      app.insert("icon".into(), ICON.into());
      app.insert("label".into(), LABEL.into());
    }
  }

  let module = document
    .get_mut("module")
    .and_then(Value::as_object_mut)
    .ok_or_else(|| anyhow::anyhow!("process_profile module.json lacks module object"))?;
  let abilities = module.entry("abilities").or_insert_with(|| Value::Array(Vec::new()));
  let abilities = abilities.as_array_mut().ok_or_else(|| {
    anyhow::anyhow!("process_profile module.json abilities is not an array")
  })?;

  let (start_window_icon, start_window_background) = abilities
    .first()
    .and_then(Value::as_object)
    .map(|first| {
      (
        first
          .get("startWindowIcon")
          .cloned()
          .unwrap_or_else(|| Value::String("startIcon".to_string())),
        first
          .get("startWindowBackground")
          .cloned()
          .unwrap_or_else(|| Value::String("#FFFFFF".to_string())),
      )
    })
    .unwrap_or_else(|| {
      (
        Value::String("startIcon".to_string()),
        Value::String("#FFFFFF".to_string()),
      )
    });

  abilities.push(serde_json::json!({
    "name": TEST_ABILITY,
    "srcEntrance": ENTRY_TEST_ABILITY_PATH,
    "startWindowIcon": start_window_icon,
    "startWindowBackground": start_window_background,
  }));
  Ok(())
}

/// `ReplaceUnitTestIndexFile.doTaskAction` — set `module.pages =
/// "$profile:main_pages"` on the res module.json (falling back to the
/// processed profile when it does not exist) and write
/// `resources/base/profile/main_pages.json` as `{ "src": ["<replace page>"] }`.
///
/// HAP modules with an existing `pages` value keep it (hvigor's `!pages`
/// guard); HAR always rewrites; HSP rewrites only when `pages` is absent.
pub(crate) fn replace_unit_test_index_file(
  res_module_json: &Path,
  process_profile: &Value,
  res_profile_dir: &Path,
  module_type: &str,
) -> Result<()> {
  let page_json_file = res_profile_dir.join("main_pages.json");
  let write_main_pages = |page_json_file: &Path| -> Result<()> {
    if let Some(parent) = page_json_file.parent() {
      std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
      page_json_file,
      serde_json::to_string_pretty(&serde_json::json!({ "src": [UNIT_TEST_REPLACE_PAGE] }))?,
    )?;
    Ok(())
  };

  if module_type == "har" {
    write_module_json(res_module_json, process_profile)?;
    return write_main_pages(&page_json_file);
  }

  let existing_pages = if res_module_json.is_file() {
    let content = std::fs::read_to_string(res_module_json)?;
    serde_json::from_str::<Value>(&content)
      .ok()
      .and_then(|doc| doc.get("module").and_then(|m| m.get("pages")).cloned())
  } else {
    None
  };
  let is_hsp = module_type == "shared";
  if !is_hsp {
    // HAP: only rewrite when pages is absent.
    if existing_pages.is_some() {
      return write_main_pages(&page_json_file);
    }
  } else if existing_pages.is_some() {
    // HSP: keep an existing pages; still emit main_pages.json.
    return write_main_pages(&page_json_file);
  }
  write_module_json(res_module_json, process_profile)?;
  write_main_pages(&page_json_file)
}

fn write_module_json(res_module_json: &Path, process_profile: &Value) -> Result<()> {
  let mut document = if res_module_json.is_file() {
    let content = std::fs::read_to_string(res_module_json)?;
    serde_json::from_str::<Value>(&content).unwrap_or_else(|_| process_profile.clone())
  } else {
    process_profile.clone()
  };
  let mut module_pages = |document: &mut Value| {
    if let Some(module) = document.get_mut("module").and_then(Value::as_object_mut) {
      module.insert("pages".into(), PAGE_JSON_RESOURCE.into());
      true
    } else {
      false
    }
  };
  if !module_pages(&mut document) {
    // Neither the res module.json nor the processed profile has a module
    // section — hvigor synthesizes `{ "module": { "pages": ... } }`.
    document = serde_json::json!({ "module": { "pages": PAGE_JSON_RESOURCE } });
  }
  if let Some(parent) = res_module_json.parent() {
    std::fs::create_dir_all(parent)?;
  }
  std::fs::write(res_module_json, serde_json::to_string_pretty(&document)?)?;
  Ok(())
}

/// `@ohos/coverage` `pageIndexTemplate` — the local unit-test entry page.
/// Placeholders: `__MODULE_PATH__`, `__BUNDLE_NAME__`, `__MODULE_NAME__`,
/// `__SCOPE__`, `__COVERAGE__`.
const PAGE_INDEX_TEMPLATE: &str = "\n    import hilog from '@ohos.hilog';\n    import AbilityDelegatorRegistry from '@ohos.app.ability.abilityDelegatorRegistry'\n\n    import {Hypium} from '@ohos/hypium'\n    import testsuite from '../../../src/test/List.test'\n\n    const global: object = new Function(\"return this\")();\n    const savePath = '__savePath__';\n    const readPath = '__readPath__';\n    const testMode: string = '__testMode__';\n    global[savePath] = \"__MODULE_PATH__/.test/default/intermediates/test/coverage_data/js_coverage.json\";\n    global[readPath] = \"__MODULE_PATH__/.test/default/intermediates/test/coverage_data/js_coverage.json\"\n    global[testMode] = 'unitTest';\n\n    @Entry\n    @Component\n    struct Index1 {\n      aboutToAppear() {\n        console.info('[LOCAL_TEST] START')\n        let abilityDelegator: AbilityDelegatorRegistry.AbilityDelegator;\n        abilityDelegator = AbilityDelegatorRegistry.getAbilityDelegator();\n        let abilityDelegatorArguments: AbilityDelegatorRegistry.AbilityDelegatorArgs;\n        abilityDelegatorArguments = AbilityDelegatorRegistry.getArguments();\n        abilityDelegatorArguments.bundleName = \"__BUNDLE_NAME__\";\n        abilityDelegatorArguments.parameters = {\n          \"-b\": \"__BUNDLE_NAME__\",\n          \"-m\": \"__MODULE_NAME__\",\n          \"-s class\": \"__SCOPE__\",\n          \"-s timeout\": \"15000\",\n          \"-s coverage\": \"__COVERAGE__\"\n        };\n        abilityDelegatorArguments.testCaseNames = \"__SCOPE__\"\n        console.info(\"[LOCAL_TEST] \" + abilityDelegatorArguments);\n        Hypium.hypiumTest(abilityDelegator, abilityDelegatorArguments, testsuite)\n        console.info('[LOCAL_TEST] END')\n      }\n    \n      @State\n      message: string = 'Hello World'\n    \n      \n  build() {\n    Row() {\n      Column() {\n        Text(this.message)\n          .fontSize(50)\n          .fontWeight(FontWeight.Bold)\n        Button() {\n          Text('next page')\n            .fontSize(20)\n            .fontWeight(FontWeight.Bold)\n        }\n        .type(ButtonType.Capsule)\n        .margin({\n          top: 20\n        })\n        .backgroundColor('#0D9FFB')\n        .width('35%')\n        .height('5%')\n        .onClick(() => {\n        })\n      }\n      .width('100%')\n    }\n    .height('100%')\n  }\n\n    }";

/// `@ohos/coverage` `testAbilityTemplate` — fixed `EntryAbility` that loads
/// the generated test page.
const TEST_ABILITY_TEMPLATE: &str = "\n    import AbilityConstant from '@ohos.app.ability.AbilityConstant';\n    import hilog from '@ohos.hilog';\n    import UIAbility from '@ohos.app.ability.UIAbility';\n    import Want from '@ohos.app.ability.Want';\n    import window from '@ohos.window';\n    \n    export default class EntryAbility extends UIAbility {\n        onCreate(want: Want, launchParam: AbilityConstant.LaunchParam) {\n            hilog.info(0x0000, 'testTag', '%{public}s', 'Ability onCreate');\n        }\n    \n        onDestroy() {\n            hilog.info(0x0000, 'testTag', '%{public}s', 'Ability onDestroy');\n        }\n    \n        onWindowStageCreate(windowStage: window.WindowStage) {\n            // Main window is created, set main page for this ability\n            hilog.info(0x0000, 'testTag', '%{public}s', 'Ability onWindowStageCreate');\n    \n            windowStage.loadContent('../../../.test/testability/pages/Index', (err, data) => {\n                if (err.code) {\n                    hilog.error(0x0000, 'testTag', 'Failed to load the content. Cause: %{public}s', JSON.stringify(err) ?? '');\n                    return;\n                }\n                hilog.info(0x0000, 'testTag', 'Succeeded in loading the content. Data: %{public}s', JSON.stringify(data) ?? '');\n            });\n        }\n    \n        onWindowStageDestroy() {\n            // Main window is destroyed, release UI related resources\n            hilog.info(0x0000, 'testTag', '%{public}s', 'Ability onWindowStageDestroy');\n        }\n    \n        onForeground() {\n            // Ability has brought to foreground\n            hilog.info(0x0000, 'testTag', '%{public}s', 'Ability onForeground');\n        }\n    \n        onBackground() {\n            // Ability has back to background\n            hilog.info(0x0000, 'testTag', '%{public}s', 'Ability onBackground');\n        }\n    }\n  ";

/// `@ohos/coverage` `phoneSettingConfigTemplate` — fixed phone settings.
const PHONE_SETTING_CONFIG_TEMPLATE: &str = "\n    {\n      \"setting\": {\n        \"1.0.1\": {\n          \"Language\": {\n            \"args\": {\n              \"Language\": \"zh_CN\"\n            }\n          }\n        }\n      },\n      \"frontend\": {\n        \"1.0.0\": {\n          \"Resolution\": {\n            \"args\": {\n              \"Resolution\": \"360*780\"\n            }\n          },\n          \"DeviceType\": {\n            \"args\": {\n              \"DeviceType\": \"phone\"\n            }\n          }\n        }\n      }\n    }\n  ";
