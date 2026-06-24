/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/assemble/assemble-hap.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as task_names_js_1 from "../../common/task-names";
import * as legacy_package_hap_js_1 from "../../legacy-tasks/legacy-package-hap";
import * as package_hap_js_1 from "../../package-hap";
import * as sign_hap_js_1 from "../../sign-hap";
import * as abstract_module_hook_task_js_1 from "../abstract-module-hook-task";
var CommonHookTask = task_names_js_1.TaskNames.CommonHookTask;
export class AssembleHap extends abstract_module_hook_task_js_1.AbstractModuleHookTask {
  constructor(a, e) {
    super(a, e, CommonHookTask.ASSEMBLE_HAP);
  }
  initTaskDepends(a) {
    this.skipSignHap(a) ? this.dependsOn(this._isFaMode ? `${a.getTargetData().getTargetName()}@${legacy_package_hap_js_1.LegacyPackageHap.name}` : `${a.getTargetData().getTargetName()}@${package_hap_js_1.PackageHap.name}`) : this.dependsOn(`${a.getTargetData().getTargetName()}@${sign_hap_js_1.SignHap.name}`);
  }
  skipSignHap(a) {
    var e;
    return (null === (e = a.getBuildOption().packOptions) || undefined === e ? undefined : e.buildAppSkipSignHap) && hvigor_1.hvigorCore.isCommandEntryTask(task_names_js_1.TaskNames.CommonHookTask.ASSEMBLE_APP.name);
  }
}
