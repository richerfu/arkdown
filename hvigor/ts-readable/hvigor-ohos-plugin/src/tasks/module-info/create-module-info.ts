/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/module-info/create-module-info.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as task_names_js_1 from "../common/task-names";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
import * as ohos_uitransform_optimization_js_1 from "./ohos-uitransform-optimization";
var __decorate = this && this.__decorate || function (e, t, o, r) {
  var s,
    i = arguments.length,
    n = i < 3 ? t : null === r ? r = Object.getOwnPropertyDescriptor(t, o) : r;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) n = Reflect.decorate(e, t, o, r);else for (var _ = e.length - 1; _ >= 0; _--) (s = e[_]) && (n = (i < 3 ? s(n) : i > 3 ? s(t, o, n) : s(t, o)) || n);
  return i > 3 && n && Object.defineProperty(t, o, n), n;
};
export const MODULE_INFO_CODE = "\n//@ts-noCheck\nexport const __MODULE_NAME__: string = __getCurrentModuleName__()\nexport const __BUNDLE_NAME__: string = __getCurrentBundleName__()\n";
export class CreateModuleInfo extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.CommonTask.CREATE_MODULE_INFO), this.outputFile = this.pathInfo.getGenerateModuleInfoPath();
  }
  beforeTask() {
    fs_extra_1.existsSync(this.outputFile) && fs_extra_1.removeSync(this.outputFile);
  }
  get ohosUiTransformOptimization() {
    return (0, ohos_uitransform_optimization_js_1.getOhosUiTransformOptimization)(this.targetService);
  }
  get integratedHspUseGetCurrentBundleName() {
    return inject_util_js_1.InjectUtil.isIntegratedHspUseGetCurrentBundleName() && this.targetService.isIntegratedHsp();
  }
  taskShouldDo() {
    return (0, ohos_uitransform_optimization_js_1.getOhosUiTransformOptimization)(this.targetService) || this.integratedHspUseGetCurrentBundleName;
  }
  async doTaskAction() {
    fs_extra_1.ensureDirSync(path_1.dirname(this.outputFile)), fs_extra_1.writeFileSync(this.outputFile, MODULE_INFO_CODE, {
      encoding: "utf-8"
    });
  }
  initTaskDepends() {}
}
__decorate([(0, hvigor_1.OutputFile)({
  isDirectory: false
})], CreateModuleInfo.prototype, "outputFile", undefined), __decorate([(0, hvigor_1.Input)()], CreateModuleInfo.prototype, "ohosUiTransformOptimization", null), __decorate([(0, hvigor_1.Input)()], CreateModuleInfo.prototype, "integratedHspUseGetCurrentBundleName", null);
