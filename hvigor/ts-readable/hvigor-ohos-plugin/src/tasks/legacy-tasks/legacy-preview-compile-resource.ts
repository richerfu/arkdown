/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-preview-compile-resource.js
 */
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as compile_resources_util_js_1 from "../../utils/compile-resources-util";
import * as har_target_util_js_1 from "../../utils/har-target-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as abstract_previewer_compile_resource_js_1 from "../abstract/abstract-previewer-compile-resource";
import * as task_names_js_1 from "../common/task-names";
import * as module_task_service_js_1 from "../service/module-task-service";
import * as legacy_hook_compile_resource_js_1 from "./hook/legacy-hook-compile-resource";
var LegacyFATask = task_names_js_1.TaskNames.LegacyFATask;
export class LegacyPreviewCompileResource extends abstract_previewer_compile_resource_js_1.AbstractPreviewerCompileResource {
  declareExecutionCommand() {
    return this.compileCommands.toString();
  }
  constructor(e, s) {
    super(e, (0, legacy_hook_compile_resource_js_1.getTaskRealName)(LegacyFATask.PREVIEW_COMPILE_RESOURCE, s)), this._log = ohos_logger_js_1.OhosLogger.getLogger(LegacyPreviewCompileResource.name), this.entryModuleName = s, this.generateSourceRDir = this.getTaskTempDir(this.targetData, build_directory_const_js_1.BuildDirConst.R, false), this.processProfileTask = (0, legacy_hook_compile_resource_js_1.getTaskRealName)(LegacyFATask.PREVIEW_PROCESS_RESOURCE, this.entryModuleName).name;
  }
  initTaskDepends() {
    const e = this.service.getRelatedEntryModules(),
      s = this.service.getModuleModel().getModuleType();
    if (module_task_service_js_1.ModuleTaskService.checkEntryModules(s, e)) for (const s of e) this.declareDepends((0, legacy_hook_compile_resource_js_1.getTaskRealName)(LegacyFATask.PREVIEW_PROCESS_RESOURCE, s).name);else this.declareDepends((0, legacy_hook_compile_resource_js_1.getTaskRealName)(LegacyFATask.PREVIEW_PROCESS_RESOURCE).name);
  }
  initHarModuleDepends() {
    har_target_util_js_1.HarTargetUtil.calDepHarTargets(this.targetService).forEach((e, s) => this.declareDepends(`${e}@${LegacyFATask.PREVIEW_COMPILE_RESOURCE.name}`, s));
  }
  async doTaskAction() {
    await this.compilationProcess(compile_resources_util_js_1.CompileResourceBuildCommandType.COMMAND, this.targetData);
  }
  async invokeRestool(e) {
    await this.previewCompileLink(this._log, async () => {
      undefined !== this.entryModuleName && (await this.retainDeviceType(e, this.entryModuleName, this._log));
    });
  }
}
