/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/debugger/hot-reload-build.js
 */
import * as task_util_js_1 from "../../utils/task-util";
import * as task_names_js_1 from "../common/task-names";
import * as abstract_module_hook_task_js_1 from "../hook/abstract-module-hook-task";
export class HotReloadBuild extends abstract_module_hook_task_js_1.AbstractModuleHookTask {
  constructor(e) {
    super(e, false, task_names_js_1.TaskNames.Task.HOT_RELOAD_BUILD);
  }
  initTaskDepends(e) {
    this.dependsOn((0, task_util_js_1.resolveHookDependency)("HotReloadArkTS", e.getTargetData().getTargetName()));
  }
}
