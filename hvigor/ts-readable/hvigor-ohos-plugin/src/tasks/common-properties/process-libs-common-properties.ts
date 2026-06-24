/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/common-properties/process-libs-common-properties.js
 */
import * as glob_1 from "glob";
import * as common_const_js_1 from "../../const/common-const";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
export class ProcessLibsCommonProperties extends ohos_hap_task_js_1.OhosHapTask {
  constructor(s, o) {
    super(s, o);
  }
  get libsPattern() {
    var s;
    return (null === (s = this.targetService.getBuildOption().nativeLib) || undefined === s ? undefined : s.collectAllLibs) ? common_const_js_1.NativeConst.LIBRARY_ALL_FILE_PATTERN : common_const_js_1.NativeConst.LIBRARY_FILE_PATTERN;
  }
  getLibsInfo(s) {
    return glob_1.glob.sync(this.libsPattern, {
      cwd: s
    });
  }
  doTaskAction() {}
  initTaskDepends() {}
}
