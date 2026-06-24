/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/ohos-test-util.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as file_util_js_1 from "./file-util";
import * as res_model_loader_js_1 from "./loader/file/res-model-loader";
import * as common_const_js_1 from "../const/common-const";
export function isTemplatesAfterSourceCodeMigration(e) {
  var t;
  const s = res_model_loader_js_1.resModelLoader.getModuleJson(path_1.resolve(e, common_const_js_1.CommonConst.MODULE_JSON5)),
    i = null === (t = null == s ? undefined : s.module) || undefined === t ? undefined : t.abilities;
  if (i && i.length) {
    return !i[0].srcEntry;
  }
  return !(file_util_js_1.FileUtil.matchingFile(path_1.resolve(e, "ets/testrunner/OpenHarmonyTestRunner.ets"), true) || file_util_js_1.FileUtil.matchingFile(path_1.resolve(e, "ets/testrunner/OpenHarmonyTestRunner.ts"), true));
}
export function getOhosTestIntermediatesSrcOhosTestRelativePath() {
  var e;
  return `.test/${null !== (e = hvigor_1.hvigor.getParameterInternal().getExtParamInternal("product")) && undefined !== e ? e : "default"}/intermediates/src/ohosTest`;
}
export function getOhosTestIntermediatesEtsPath(e) {
  return path_1.resolve(e, getOhosTestIntermediatesSrcOhosTestRelativePath(), "ets");
}
export function getOhosTestAbilityConfig() {
  return {
    description: "$string:module_test_desc",
    mainElement: "TestAbility",
    pages: "$profile:test_pages",
    abilities: [{
      name: "TestAbility",
      srcEntry: "./ets/testability/TestAbility.ets",
      description: "$string:TestAbility_desc",
      label: "$string:TestAbility_label",
      exported: true
    }]
  };
}
