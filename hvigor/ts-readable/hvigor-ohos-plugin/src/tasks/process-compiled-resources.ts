/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-compiled-resources.js
 */
import path_1 from "path";
import fs_extra_1 from "fs-extra";
import * as hvigor_1 from "@ohos/hvigor";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
import * as task_names_js_1 from "./common/task-names";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as task_util_js_1 from "../utils/task-util";
var __decorate = this && this.__decorate || function (e, s, t, o) {
  var r,
    a = arguments.length,
    i = a < 3 ? s : null === o ? o = Object.getOwnPropertyDescriptor(s, t) : o;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) i = Reflect.decorate(e, s, t, o);else for (var _ = e.length - 1; _ >= 0; _--) (r = e[_]) && (i = (a < 3 ? r(i) : a > 3 ? r(s, t, i) : r(s, t)) || i);
  return a > 3 && i && Object.defineProperty(s, t, i), i;
};
var Task = task_names_js_1.TaskNames.Task;
export class ProcessCompiledResources extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, Task.PROCESS_COMPILED_RESOURCES), this.preloadSoJsonPath = this.pathInfo.getPreloadSoFilePath(), this.resourcePath = path_1.resolve(this.pathInfo.getIntermediatesRes(), build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES);
  }
  taskShouldDo() {
    return !(0, task_util_js_1.isFastBuildApp)(this.service) && this.isPreloadSystemSoJsonEffective && fs_extra_1.existsSync(this.resourcePath);
  }
  beforeTask() {
    super.beforeTask();
    const e = this.getTaskTempDir(this.targetData, undefined, false);
    fs_extra_1.existsSync(e) && fs_extra_1.removeSync(e);
  }
  processPreloadSystemSoJson() {
    const e = this.getTaskTempDir(this.targetData);
    fs_extra_1.emptyDirSync(e);
    const s = path_1.resolve(e, path_1.basename(this.resourcePath));
    fs_extra_1.copySync(this.resourcePath, s), this.addPreloadSystemSoJson(this.getPreloadSystemSoJson(), e);
  }
  initTaskDepends() {}
  doTaskAction() {
    this.processPreloadSystemSoJson();
  }
}
__decorate([(0, hvigor_1.Input)()], ProcessCompiledResources.prototype, "resourcePath", undefined), __decorate([(0, hvigor_1.InputFile)()], ProcessCompiledResources.prototype, "preloadSoJsonPath", undefined);
