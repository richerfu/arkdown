/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/unit-test/legacy-unit-test-ark-compile.js
 */
import path_1 from "path";
import * as ohos_logger_js_1 from "../../../utils/log/ohos-logger";
import * as legacy_ark_compile_js_1 from "../legacy-ark-compile";
import * as build_unit_test_hook_js_1 from "../../unitTest/build-unit-test-hook";
export class LegacyUnitTestArkCompile extends legacy_ark_compile_js_1.LegacyArkCompile {
  constructor() {
    super(...arguments), this.logger = ohos_logger_js_1.OhosLogger.getLogger(LegacyUnitTestArkCompile.name);
  }
  declareInputFiles() {
    return super.declareInputFiles();
  }
  async initDefaultArkCompileConfig() {
    return {
      ...(await super.initDefaultArkCompileConfig()),
      aceModuleJsonPath: undefined,
      aceModuleBuild: path_1.resolve(this.pathInfo.getInterMediatesAssetsPath(), this.codeType)
    };
  }
  initTaskDepends() {
    this.declareDepends(build_unit_test_hook_js_1.BuildUnitTestHook.name);
  }
  async doTaskAction() {
    await super.doTaskAction();
  }
}
