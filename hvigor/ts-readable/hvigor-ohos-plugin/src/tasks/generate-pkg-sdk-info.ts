/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/generate-pkg-sdk-info.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
import * as file_util_js_1 from "../utils/file-util";
import * as task_names_js_1 from "./common/task-names";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
var __decorate = this && this.__decorate || function (e, t, s, r) {
  var a,
    o = arguments.length,
    n = o < 3 ? t : null === r ? r = Object.getOwnPropertyDescriptor(t, s) : r;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) n = Reflect.decorate(e, t, s, r);else for (var i = e.length - 1; i >= 0; i--) (a = e[i]) && (n = (o < 3 ? a(n) : o > 3 ? a(t, s, n) : a(t, s)) || n);
  return o > 3 && n && Object.defineProperty(t, s, n), n;
};
export class GeneratePkgSdkInfo extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.Task.GENERATE_PKG_SDK_INFO), this.pkgSdkInfoMap = new Map(), this.pkgSdkInfoPath = this.pathInfo.getIntermediatesPkgSdkInfoPath();
  }
  get useNormalizedOHMUrl() {
    var e, t;
    return !!(null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.strictMode) || undefined === t ? undefined : t.useNormalizedOHMUrl);
  }
  async beforeAlwaysAction() {
    return this.initPkgSdkInfo(), super.beforeAlwaysAction();
  }
  beforeTask() {
    fs_extra_1.existsSync(this.pkgSdkInfoPath) && fs_extra_1.removeSync(this.pkgSdkInfoPath), super.beforeTask();
  }
  declareInputs() {
    const e = super.declareInputs();
    return e.set("pkgSdkInfo", JSON.stringify(Object.fromEntries(this.pkgSdkInfoMap))), e;
  }
  declareOutputFiles() {
    const e = super.declareOutputFiles();
    return e.addEntry(this.pkgSdkInfoPath, {
      isDirectory: false
    }), e;
  }
  taskShouldDo() {
    return !this.useNormalizedOHMUrl;
  }
  doTaskAction() {
    fs_extra_1.existsSync(path_1.dirname(this.pkgSdkInfoPath)) || fs_extra_1.mkdirSync(path_1.dirname(this.pkgSdkInfoPath), {
      recursive: true
    });
    const e = JSON.stringify(Object.fromEntries(this.pkgSdkInfoMap));
    fs_extra_1.writeFileSync(this.pkgSdkInfoPath, e);
  }
  initTaskDepends() {}
  initPkgSdkInfo() {
    const e = new dependency_manager_js_1.DependencyManager(this.projectModel.isFaMode(), this.moduleModel, this.projectModel),
      {
        npmDependencies: t
      } = e.collectAllDependenciesForPkgInfo(true),
      s = this.targetService.getModuleService().getDependencyInfo().getDirectPkgNodes().map(e => e.npm.getPackageName());
    t.forEach(e => {
      const t = e.getDependencyType(),
        r = s.includes(e.getPackageName());
      this.pkgSdkInfoMap.set(e.getPackageName(), {
        version: this.getVersion(e),
        type: "hsp" === t ? "shared" : t,
        isDirectDep: r
      });
    }), this.pkgSdkInfoMap.set(this.packageJsonObj.name, {
      version: this.packageJsonObj.version,
      type: this.moduleModel.getModuleType(),
      isDirectDep: false
    });
  }
  getVersion(e) {
    const t = {
        parameterFilePath: this.service.getProjectModel().getParameterFileAbsolutePath(),
        ohPackageFilePath: this.isOhpmProject ? this.moduleModel.getOhPackageJson5Path() : this.moduleModel.getPackageJsonPath()
      },
      s = undefined === e.getDependencyVersion() ? "" : `${e.getDependencyVersion()}`;
    return file_util_js_1.FileUtil.extracted(s, this.moduleModel.getParentProject().getParameterFileObj(), t);
  }
}
__decorate([(0, hvigor_1.Input)()], GeneratePkgSdkInfo.prototype, "useNormalizedOHMUrl", null);
