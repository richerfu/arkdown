/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/generate-pkg-module-json.js
 */
import * as collect_duplicate_har_dependencies_js_1 from "../har-dedepulicate/collect_duplicate_har_dependencies";
import * as task_names_js_1 from "./common/task-names";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as common_const_js_1 from "../const/common-const";
import * as json_util_js_1 from "../utils/json-util";
import * as build_directory_const_js_1 from "../const/build-directory-const";
var __decorate = this && this.__decorate || function (e, t, o, s) {
  var a,
    r = arguments.length,
    i = r < 3 ? t : null === s ? s = Object.getOwnPropertyDescriptor(t, o) : s;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) i = Reflect.decorate(e, t, o, s);else for (var n = e.length - 1; n >= 0; n--) (a = e[n]) && (i = (r < 3 ? a(i) : r > 3 ? a(t, o, i) : a(t, o)) || i);
  return r > 3 && i && Object.defineProperty(t, o, i), i;
};
var Task = task_names_js_1.TaskNames.Task;
export class GeneratePkgModuleJson extends ohos_hap_task_js_1.OhosHapTask {
  shouldDeduplicateHar() {
    return this.targetService.shouldEnableHarDeduplicate();
  }
  constructor(e) {
    super(e, Task.GENERATE_PKG_MODULE_JSON);
    const t = this.targetService.getTargetData().getPathInfo();
    this.jsonPath = path_1.resolve(t.getIntermediatesRes(), common_const_js_1.CommonConst.MODULE_JSON), this.packageHapJsonPath = path_1.resolve(this.pathInfo.getPackageHapProductPath(), common_const_js_1.CommonConst.MODULE_JSON), this.preloadSoJsonPath = this.pathInfo.getPreloadSoFilePath();
  }
  ensureAppStartup(e) {
    e.module.appStartup || (e.module.appStartup = `$profile:${build_directory_const_js_1.BuildDirConst.DEFAULT_STARTUP_CONFIG_FILE_NAME}`);
  }
  writeDeduplicateHarField(e) {
    if (!this.moduleModel.isHspModule()) return;
    const t = (0, collect_duplicate_har_dependencies_js_1.getHspNeedDeduplicateHarInfoMap)(this.moduleName);
    e.module.deduplicateHar = !!(this.targetService.shouldDoDeduplicateTask() && t && Object.keys(t).length > 0);
  }
  async doTaskAction() {
    const e = this.pathInfo.getPackageHapProductPath();
    if (fs_extra_1.existsSync(e) || fs_extra_1.ensureDirSync(e), fs_extra_1.existsSync(this.jsonPath)) {
      const e = json_util_js_1.JsonUtil.getJson5Obj(this.jsonPath);
      this.isPreloadSystemSoJsonEffective && e && this.ensureAppStartup(e), this.shouldDeduplicateHar() && e && this.writeDeduplicateHarField(e), fs_extra_1.writeFileSync(this.packageHapJsonPath, JSON.stringify(e));
    }
  }
  initTaskDepends() {}
}
__decorate([(0, hvigor_1.InputFile)()], GeneratePkgModuleJson.prototype, "jsonPath", undefined), __decorate([(0, hvigor_1.OutputFile)()], GeneratePkgModuleJson.prototype, "packageHapJsonPath", undefined), __decorate([(0, hvigor_1.InputFile)()], GeneratePkgModuleJson.prototype, "preloadSoJsonPath", undefined), __decorate([(0, hvigor_1.Input)()], GeneratePkgModuleJson.prototype, "shouldDeduplicateHar", null);
