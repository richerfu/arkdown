/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/unitTest/test.js
 */
import * as common_const_js_1 from "../../const/common-const";
import * as task_util_js_1 from "../../utils/task-util";
import * as task_names_js_1 from "../common/task-names";
import * as abstract_module_hook_task_js_1 from "../hook/abstract-module-hook-task";
export class Test extends abstract_module_hook_task_js_1.AbstractModuleHookTask {
  constructor(s, t) {
    super(s, t, task_names_js_1.TaskNames.Task.TEST);
  }
  initTaskDepends(s) {
    const t = this._isFaMode ? common_const_js_1.ArktSTaskConst.LEGACY_UNIT_TEST_ARKTS : common_const_js_1.ArktSTaskConst.UNIT_TEST_ARKTS;
    this.dependsOn((0, task_util_js_1.resolveHookDependency)(t, s.getTargetData().getTargetName()));
  }
}
