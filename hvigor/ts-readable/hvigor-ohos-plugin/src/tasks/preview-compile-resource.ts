/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/preview-compile-resource.js
 */
import fs_extra_1 from "fs-extra";
import * as compile_resources_util_js_1 from "../utils/compile-resources-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as task_util_js_1 from "../utils/task-util";
import * as abstract_previewer_compile_resource_js_1 from "./abstract/abstract-previewer-compile-resource";
import * as task_names_js_1 from "./common/task-names";
var Task = task_names_js_1.TaskNames.Task;
export class PreviewCompileResource extends abstract_previewer_compile_resource_js_1.AbstractPreviewerCompileResource {
  declareExecutionCommand() {
    return this.compileCommands.toString();
  }
  constructor(e) {
    super(e, Task.PREVIEW_COMPILE_RESOURCE), this._log = ohos_logger_js_1.OhosLogger.getLogger(PreviewCompileResource.name), this.generateSourceRDir = this.pathInfo.getGenerateSourceR(), this.processProfileTask = Task.PREVIEW_PROCESS_RESOURCE.name;
  }
  initTaskDepends() {
    this.declareDepends((0, task_util_js_1.resolveHookDependency)(Task.PREVIEW_PROCESS_RESOURCE.name, this.targetName));
  }
  declareInputFiles() {
    const e = this.targetService.getTargetData().getPathInfo().getIntermediatesMergeProfile();
    return fs_extra_1.existsSync(e) ? super.declareInputFiles().addEntry(e) : super.declareInputFiles();
  }
  async doTaskAction() {
    await this.compilationProcess(compile_resources_util_js_1.CompileResourceBuildCommandType.COMMAND, this.targetData);
  }
  async invokeRestool(e) {
    await this.previewCompileLink(this._log, () => {
      this.replaceTargetPages(e, this._log);
    });
  }
}
