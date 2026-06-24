/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/previewer/preview-build.js
 */
import * as task_util_js_1 from "../../../utils/task-util";
import * as task_names_js_1 from "../../common/task-names";
import * as abstract_module_hook_task_js_1 from "../abstract-module-hook-task";
export class PreviewBuild extends abstract_module_hook_task_js_1.AbstractModuleHookTask {
  constructor(e, s) {
    super(e, s, task_names_js_1.TaskNames.Task.PREVIEW_BUILD);
  }
  initTaskDepends(e) {
    const s = this._isFaMode ? "LegacyPreviewCompileArkTS" : "PreviewArkTS";
    this.dependsOn((0, task_util_js_1.resolveHookDependency)(s, e.getTargetData().getTargetName()));
  }
}
