/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/ohosTest/ohos-test-ark-compile.js
 */
import * as fse from "fs-extra";
import * as task_names_js_1 from "../common/task-names";
import * as ark_compile_js_1 from "../ark-compile";
import * as common_const_js_1 from "../../const/common-const";
var Task = task_names_js_1.TaskNames.Task;
export class OhosTestArkCompile extends ark_compile_js_1.ArkCompile {
  constructor(e, t) {
    super(e, t, Task.OHOS_TEST_ARK_COMPILE);
  }
  declareInputFiles() {
    const e = super.declareInputFiles();
    return this.mainCodePath && fse.existsSync(this.mainCodePath) && e.addEntry(this.mainCodePath, {
      isDirectory: true
    }), e;
  }
  allowToPreloadSystemSo() {
    return false;
  }
  async initDefaultArkCompileConfig() {
    const e = {
      ...(await super.initDefaultArkCompileConfig()),
      supportChunks: false
    };
    return e.obfuscationOptions && delete e.obfuscationOptions, e.testFrameworkPar && (e.testFrameworkPar.testMode = common_const_js_1.CommonConst.OHOS_TEST), e;
  }
}
