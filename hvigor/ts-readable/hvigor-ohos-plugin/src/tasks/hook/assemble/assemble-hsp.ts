/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/assemble/assemble-hsp.js
 */
import * as task_names_js_1 from "../../common/task-names";
import * as package_shared_tgz_js_1 from "../../hsp/package-shared-tgz";
import * as abstract_module_hook_task_js_1 from "../abstract-module-hook-task";
var CommonHookTask = task_names_js_1.TaskNames.CommonHookTask;
export class AssembleHsp extends abstract_module_hook_task_js_1.AbstractModuleHookTask {
  constructor(s, e) {
    super(s, e, CommonHookTask.ASSEMBLE_HSP);
  }
  initTaskDepends(s) {
    const e = `${s.getTargetData().getTargetName()}@${package_shared_tgz_js_1.PackageSharedTgz.name}`;
    this.dependsOn(e);
  }
}
