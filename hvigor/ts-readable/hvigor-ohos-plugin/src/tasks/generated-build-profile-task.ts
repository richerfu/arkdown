/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/generated-build-profile-task.js
 */
import * as task_names_js_1 from "./common/task-names";
import * as abstract_module_hook_task_js_1 from "./hook/abstract-module-hook-task";
var CommonTask = task_names_js_1.TaskNames.CommonTask,
  CommonHookTask = task_names_js_1.TaskNames.CommonHookTask;
export class GeneratedBuildProfileTask extends abstract_module_hook_task_js_1.AbstractModuleHookTask {
  constructor(s, e) {
    super(s, false, CommonHookTask.GENERATE_BUILD_PROFILE), this.isHar = false, this.isHar = e;
  }
  initTaskDepends(s) {
    this.isHar ? this.dependsOn(`${s.getTargetData().getTargetName()}@${CommonTask.CREATE_HAR_BUILD_PROFILE.name}`) : this.dependsOn(`${s.getTargetData().getTargetName()}@${CommonTask.CREATE_BUILD_PROFILE.name}`);
  }
}
