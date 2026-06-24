/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/hook/legacy-hook-compile-resource.js
 */
import * as task_names_js_1 from "../../common/task-names";
import * as ohos_hap_task_js_1 from "../../task/ohos-hap-task";
import * as legacy_compile_resource_js_1 from "../legacy-compile-resource";
import * as legacy_preview_compile_resource_1 from "../legacy-preview-compile-resource";
import * as wdk_1 from "@baize/wdk";
import * as module_task_service_js_1 from "../../service/module-task-service";
var CommonTask = task_names_js_1.TaskNames.CommonTask;
export class LegacyHookCompileResource extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, CommonTask.LEGACY_HOOK_COMPILE_RESOURCE);
  }
  doTaskAction() {}
  initTaskDepends() {
    const e = this.sdkInfo.isPreviewCompileResourceIncrement(this.targetData.isHarmonyOS()) ? legacy_preview_compile_resource_1.LegacyPreviewCompileResource.name : legacy_compile_resource_js_1.LegacyCompileResource.name,
      s = this.moduleModel.getModuleType(),
      o = this.moduleModel.getRelatedEntryModules();
    if (module_task_service_js_1.ModuleTaskService.checkEntryModules(s, o)) for (const s of this.service.getRelatedEntryModules()) this.declareDepends(getTaskRealName({
      name: e
    }, s).name);else this.declareDepends(getTaskRealName({
      name: e
    }).name);
  }
}
export function getTaskRealName(e, s) {
  return s ? {
    ...e,
    name: `${e.name}With${(0, wdk_1.capitalize)(s)}`
  } : e;
}
