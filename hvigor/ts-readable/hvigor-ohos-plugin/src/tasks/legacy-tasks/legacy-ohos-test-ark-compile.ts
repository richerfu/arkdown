/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-ohos-test-ark-compile.js
 */
import fs_extra_1 from "fs-extra";
import * as task_names_js_1 from "../common/task-names";
import * as legacy_ark_compile_js_1 from "./legacy-ark-compile";
import * as common_const_js_1 from "../../const/common-const";
export class LegacyOhosTestArkCompile extends legacy_ark_compile_js_1.LegacyArkCompile {
  constructor(e, s) {
    super(e, s, task_names_js_1.TaskNames.LegacyFATask.OHOS_TEST_ARK_COMPILE);
  }
  declareInputFiles() {
    const e = super.declareInputFiles();
    return this.mainCodePath && fs_extra_1.existsSync(this.mainCodePath) && e.addEntry(this.mainCodePath, {
      isDirectory: true
    }), e;
  }
  initDefaultLegacyArkConfig(e, s, t) {
    const r = {
      ...super.initDefaultLegacyArkConfig(e, s, t),
      supportChunks: false
    };
    return r.testFrameworkPar && (r.testFrameworkPar.testMode = common_const_js_1.CommonConst.OHOS_TEST), r;
  }
}
