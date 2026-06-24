/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/module-info/create-har-module-info.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as task_names_js_1 from "../common/task-names";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
import * as create_module_info_js_1 from "./create-module-info";
import * as ohos_uitransform_optimization_js_1 from "./ohos-uitransform-optimization";
var __decorate = this && this.__decorate || function (e, t, o, r) {
  var i,
    a = arguments.length,
    s = a < 3 ? t : null === r ? r = Object.getOwnPropertyDescriptor(t, o) : r;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) s = Reflect.decorate(e, t, o, r);else for (var _ = e.length - 1; _ >= 0; _--) (i = e[_]) && (s = (a < 3 ? i(s) : a > 3 ? i(t, o, s) : i(t, o)) || s);
  return a > 3 && s && Object.defineProperty(t, o, s), s;
};
export class CreateHarModuleInfo extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.CommonTask.CREATE_HAR_MODULE_INFO), this.outputFile = this.pathInfo.getGenerateModuleInfoPath();
  }
  get isByteCodeHar() {
    return this.targetService.isByteCodeHar();
  }
  get isOhosUiTransformOptimization() {
    return (0, ohos_uitransform_optimization_js_1.getOhosUiTransformOptimization)(this.targetService);
  }
  taskShouldDo() {
    return this.targetService.isByteCodeHar() || (0, ohos_uitransform_optimization_js_1.getOhosUiTransformOptimization)(this.targetService);
  }
  beforeTask() {
    fs_extra_1.existsSync(this.outputFile) && fs_extra_1.removeSync(this.outputFile);
  }
  async doTaskAction() {
    fs_extra_1.ensureDirSync(path_1.dirname(this.outputFile)), fs_extra_1.writeFileSync(this.outputFile, create_module_info_js_1.MODULE_INFO_CODE, {
      encoding: "utf-8"
    });
  }
  initTaskDepends() {}
}
__decorate([(0, hvigor_1.OutputFile)({
  isDirectory: false
})], CreateHarModuleInfo.prototype, "outputFile", undefined), __decorate([(0, hvigor_1.Input)()], CreateHarModuleInfo.prototype, "isByteCodeHar", null), __decorate([(0, hvigor_1.Input)()], CreateHarModuleInfo.prototype, "isOhosUiTransformOptimization", null);
