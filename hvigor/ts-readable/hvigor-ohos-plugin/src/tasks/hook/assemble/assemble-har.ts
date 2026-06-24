/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/assemble/assemble-har.js
 */
import * as task_names_js_1 from "../../common/task-names";
import * as package_har_js_1 from "../../har/package-har";
import * as legacy_package_har_js_1 from "../../legacy-tasks/har/legacy-package-har";
import * as abstract_module_hook_task_js_1 from "../abstract-module-hook-task";
var CommonHookTask = task_names_js_1.TaskNames.CommonHookTask;
export class AssembleHar extends abstract_module_hook_task_js_1.AbstractModuleHookTask {
  constructor(a, e) {
    super(a, e, CommonHookTask.ASSEMBLE_HAR);
  }
  initTaskDepends(a) {
    const e = this._isFaMode ? legacy_package_har_js_1.LegacyPackageHar.name : package_har_js_1.PackageHar.name;
    this.dependsOn(`${a.getTargetData().getTargetName()}@${e}`);
  }
}
