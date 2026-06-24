/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hook/preview-hook-compile-resource.js
 */
import * as task_names_js_1 from "../common/task-names";
import * as compile_resource_js_1 from "../compile-resource";
import * as preview_compile_resource_js_1 from "../preview-compile-resource";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
var CommonTask = task_names_js_1.TaskNames.CommonTask;
export class PreviewHookCompileResource extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, CommonTask.PREVIEW_HOOK_COMPILE_RESOURCE);
  }
  doTaskAction() {}
  initTaskDepends() {
    this.declareDepends(this.sdkInfo.isPreviewCompileResourceIncrement(this.targetData.isHarmonyOS()) ? preview_compile_resource_js_1.PreviewCompileResource.name : compile_resource_js_1.CompileResource.name);
  }
}
