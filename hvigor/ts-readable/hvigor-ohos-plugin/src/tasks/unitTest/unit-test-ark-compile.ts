/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/unitTest/unit-test-ark-compile.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as ark_compile_js_1 from "../ark-compile";
import * as task_names_js_1 from "../common/task-names";
import * as build_unit_test_hook_js_1 from "./build-unit-test-hook";
import * as unit_test_process_profile_js_1 from "./unit-test-process-profile";
import * as common_const_js_1 from "../../const/common-const";
export class UnitTestArkCompile extends ark_compile_js_1.ArkCompile {
  constructor() {
    super(...arguments), this.logger = ohos_logger_js_1.OhosLogger.getLogger(UnitTestArkCompile.name);
  }
  declareInputFiles() {
    return super.declareInputFiles();
  }
  allowToPreloadSystemSo() {
    return false;
  }
  async doTaskAction() {
    await super.doTaskAction();
  }
  async initDefaultArkCompileConfig() {
    var e;
    this.flushBuildConfig();
    const t = {
        ...(await super.initDefaultArkCompileConfig()),
        aceModuleBuild: path_1.resolve(this.pathInfo.getInterMediatesAssetsPath(), this.codeType)
      },
      s = inject_util_js_1.InjectUtil.getEntryModulesName();
    return s.length && (t.mainModulePath = null === (e = hvigor_1.hvigor.getNodeByNameInternal(s[0])) || undefined === e ? undefined : e.getNodePathInternal()), t.obfuscationOptions && delete t.obfuscationOptions, t.testFrameworkPar && (t.testFrameworkPar.testMode = common_const_js_1.CommonConst.UNIT_TEST), t;
  }
  initTaskDepends() {
    this.declareDependsList(build_unit_test_hook_js_1.BuildUnitTestHook.name, unit_test_process_profile_js_1.UnitTestProcessProfile.name);
  }
  flushBuildConfig() {
    if (this.moduleModel.isHspModule() && hvigor_1.hvigor.isCommandEntryTaskInternal(task_names_js_1.TaskNames.Task.TEST.name)) {
      const e = this.pathInfo.getTestBuildConfigPath();
      fs_extra_1.ensureDirSync(path_1.dirname(e)), fs_extra_1.writeJSONSync(e, {
        aceModuleBuild: path_1.resolve(this.pathInfo.getInterMediatesAssetsPath(), this.codeType)
      });
    }
  }
}
