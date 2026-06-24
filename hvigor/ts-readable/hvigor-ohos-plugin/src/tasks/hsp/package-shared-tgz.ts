/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hsp/package-shared-tgz.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as fse from "fs-extra";
import * as tar_1 from "tar";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_names_js_1 from "../common/task-names";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
import * as package_shared_har_js_1 from "./package-shared-har";
import * as sign_hsp_js_1 from "./sign-hsp";
var __decorate = this && this.__decorate || function (e, t, a, s) {
  var r,
    i = arguments.length,
    h = i < 3 ? t : null === s ? s = Object.getOwnPropertyDescriptor(t, a) : s;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) h = Reflect.decorate(e, t, a, s);else for (var o = e.length - 1; o >= 0; o--) (r = e[o]) && (h = (i < 3 ? r(h) : i > 3 ? r(t, a, h) : r(t, a)) || h);
  return i > 3 && h && Object.defineProperty(t, a, h), h;
};
var Task = task_names_js_1.TaskNames.Task;
export class PackageSharedTgz extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    var t, a;
    super(e, Task.PACKAGE_SHARED_TGZ), this.RELEASE = "Release", this._log = ohos_logger_js_1.OhosLogger.getLogger(PackageSharedTgz.name), this._taskService = e, this.outputDir = this.pathInfo.getModuleBuildOutputPath(), this.isRelease = e.getBuildMode() === this.RELEASE, this.tgzOutPath = path_1.resolve(this.outputDir, this.targetData.getModuleTargetOutputFileName("", false, build_directory_const_js_1.BuildArtifactExtension.DOT_TGZ)).replace("-unsigned", "");
    const s = null === (t = this.targetData.getTargetOpt().output) || undefined === t ? undefined : t.artifactName,
      r = s ? `${s}` : `${this.moduleName}`;
    this.sharedHspName = this.targetData.getModuleTargetOutputFileName("", false, build_directory_const_js_1.BuildArtifactExtension.DOT_HSP), this.sharedHspRename = this.sharedHspName.replace("-unsigned", ""), this.sharedHarName = `${r}.har`, this.tempSharedHspPath = path_1.resolve(this.pathInfo.getIntermediatesTemp(), this.sharedHspRename), this.tempSharedHarPath = path_1.resolve(this.pathInfo.getIntermediatesTemp(), this.sharedHarName);
    const i = !!(null === (a = this.targetService.getBuildOption().arkOptions) || undefined === a ? undefined : a.integratedHsp);
    this.sharedHspPath = path_1.resolve(i ? this.pathInfo.getIntegratedHspOutputPath() : this.outputDir, this.sharedHspName), this.sharedHarPath = path_1.resolve(this.outputDir, this.sharedHarName);
  }
  get generateSharedTgz() {
    var e;
    return null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.generateSharedTgz;
  }
  declareInputFiles() {
    const e = new hvigor_1.FileSet();
    return fse.existsSync(this.sharedHspPath) && e.addEntry(this.sharedHspPath), fse.existsSync(this.sharedHarPath) && e.addEntry(this.sharedHarPath), e;
  }
  declareOutputFiles() {
    return super.declareOutputFiles().addEntry(this.tgzOutPath, {
      isDirectory: false
    });
  }
  initTaskDepends() {
    const e = `${this._taskService.getTargetData().getTargetName()}@${sign_hsp_js_1.SignHsp.name}`,
      t = `${this._taskService.getTargetData().getTargetName()}@${package_shared_har_js_1.PackageSharedHar.name}`;
    this.declareDepends(e), this.declareDepends(t);
  }
  beforeTask() {
    super.beforeTask(), undefined !== this.generateSharedTgz && fse.existsSync(this.tgzOutPath) && fse.removeSync(this.tgzOutPath);
  }
  taskShouldDo() {
    return undefined === this.generateSharedTgz ? this.isRelease : this.generateSharedTgz;
  }
  async doTaskAction() {
    await this.buildTgzPackage();
  }
  async buildTgzPackage() {
    this.copyTempFiles(this.sharedHspPath, this.tempSharedHspPath), this.copyTempFiles(this.sharedHarPath, this.tempSharedHarPath), await (0, tar_1.create)({
      cwd: this.pathInfo.getIntermediatesTemp(),
      noDirRecurse: false,
      file: this.tgzOutPath,
      gzip: true
    }, [this.sharedHspRename, this.sharedHarName]), this.removeTempFiles();
  }
  copyTempFiles(e, t) {
    fse.existsSync(e) || this._log.printErrorExit("PACK_TGZ_PACKAGE_FILE_NOT_EXIST", [e]), fse.existsSync(this.pathInfo.getIntermediatesTemp()) || fse.mkdirsSync(this.pathInfo.getIntermediatesTemp()), fse.copyFileSync(e, t);
  }
  removeTempFiles() {
    fse.removeSync(this.tempSharedHspPath), fse.removeSync(this.tempSharedHarPath);
  }
}
__decorate([(0, hvigor_1.Input)()], PackageSharedTgz.prototype, "generateSharedTgz", null);
