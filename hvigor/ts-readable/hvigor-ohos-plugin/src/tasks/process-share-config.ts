/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-share-config.js
 */
import fs_extra_1 from "fs-extra";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as module_type_enum_js_1 from "../enum/module-type-enum";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as task_names_js_1 from "./common/task-names";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
import * as array_util_js_1 from "../utils/array-util";
import * as dependency_util_js_1 from "../utils/dependency-util";
export class ProcessShareConfig extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    var t;
    super(e, task_names_js_1.TaskNames.Task.PROCESS_SHARE_CONFIG), this._log = ohos_logger_js_1.OhosLogger.getLogger(ProcessShareConfig.name), this.shareConfigFileName = this.targetService.getShareConfigJsonFileName(this.moduleModel), this.intermediatesTempShareConfigFilePath = this.pathInfo.getIntermediatesShareConfig(null !== (t = this.shareConfigFileName) && undefined !== t ? t : build_directory_const_js_1.BuildArtifactConst.TEMP_SHARE_CONFIG_FILE_NAME);
  }
  declareInputFiles() {
    const e = super.declareInputFiles(),
      t = this.projectModel.isOhpmProject();
    e.addEntry(t ? this.moduleModel.getOhPackageJson5Path() : this.moduleModel.getPackageJsonPath()).addEntry(t ? this.projectModel.getOhPackageJson5Path() : this.projectModel.getPackageJsonPath()).addEntry(this.moduleModel.getJsonPathByTargetName(this.targetName));
    const s = this.targetService.getShareConfigJsonPath(this.moduleModel);
    return s && fs_extra_1.existsSync(s) && e.addEntry(s), fs_extra_1.existsSync(this.pathInfo.getIntermediatesPkgContextInfoPath()) && e.addEntry(this.pathInfo.getIntermediatesPkgContextInfoPath()), e;
  }
  declareOutputFiles() {
    return super.declareOutputFiles().addEntry(this.intermediatesTempShareConfigFilePath);
  }
  declareInputs() {
    const e = super.declareInputs();
    return this.service.getHarDependencies().forEach(t => {
      if (t.isLocal()) {
        const s = this.getLocalDependencyShareConfig(t);
        s && e.set(`${t.getDependencyName()} localDependencyShareConfigObjList`, JSON.stringify(s));
      } else {
        const s = dependency_manager_js_1.DependencyManager.getRemoteDependencyShareConfigObjList(t);
        s && e.set(`${t.getDependencyName()} remoteDependencyShareConfigObjList`, JSON.stringify(s));
      }
    }), e;
  }
  async beforeAlwaysAction() {
    await super.beforeAlwaysAction();
  }
  doTaskAction() {
    const e = this.targetService.getTargetShareConfigObjList(this.moduleModel);
    this.service.getModuleModel().getModuleType() !== module_type_enum_js_1.ModuleType.Har || this.targetService.isBundledDependencies() ? this.processHapOrHspShareConfig(e) : this.processHarShareConfig(e);
  }
  processHarShareConfig(e) {
    e && (this.checkShareConfigObjUri(e), this.generateTempshareConfig(e));
  }
  processHapOrHspShareConfig(e) {
    const t = [];
    this.processModuleSelfShareConfig(t, e), this.processModuleDependencyShareConfig(t), this.checkShareConfigObjUri(t), this.generateTempshareConfig(t);
  }
  processModuleSelfShareConfig(e, t) {
    t && (t.forEach(e => {
      e.packageName = e.packageName || this.moduleModel.getName();
    }), e.push(...t));
  }
  processModuleDependencyShareConfig(e) {
    const t = new Map();
    this.service.getHarDependencies().forEach(s => {
      s.isLocal() ? this.processLocalDependencyShareConfig(s, e, t) : this.processRemoteDependencyShareConfig(s, e);
    });
  }
  getLocalDependencyShareConfig(e) {
    return (0, dependency_util_js_1.getLocalDependencyShareConfig)(this.projectModel, this.targetService, e);
  }
  processLocalDependencyShareConfig(e, t, s) {
    const r = dependency_manager_js_1.DependencyManager.getModuleModelByDependency(e, this.projectModel),
      a = this.getLocalDependencyShareConfig(e);
    r && a && !s.has(r) && (s.set(r, a), a.forEach(t => {
      this.shareConfigAddPackageName(t, e);
    }), t.push(...a));
  }
  processRemoteDependencyShareConfig(e, t) {
    const s = dependency_manager_js_1.DependencyManager.getRemoteDependencyShareConfigObjList(e);
    s && (s.forEach(t => {
      this.shareConfigAddPackageName(t, e);
    }), t.push(...s));
  }
  shareConfigAddPackageName(e, t) {
    e.packageName = e.packageName || t.getPackageName();
  }
  checkShareConfigObjUri(e) {
    if (!e || e.length <= 1) return;
    const t = new Map();
    e.forEach(e => {
      const s = e.packageName;
      if (t.has(s)) {
        const r = t.get(s);
        if (!r) return;
        r.includes(e.uri) || (r.push(e.uri), t.set(s, r));
      } else t.set(s, [e.uri]);
    });
    const s = new Map();
    t.forEach((e, r) => {
      const a = (0, array_util_js_1.findDuplicateElement)(e);
      t.forEach((t, s) => {
        if (s === r) return;
        const n = (0, array_util_js_1.getIntersection)(t, e);
        0 !== n.length && a.push(...n);
      }), 0 !== a.length && s.set(r, Array.from(new Set(a)));
    }), 0 !== s.size && this._log.printErrorExit("DUPLICATE_SHARECONFIG_OBJECT_URI_DETECTED", undefined, [[JSON.stringify(Object.fromEntries(s.entries()))]]);
  }
  initTaskDepends() {}
  generateTempshareConfig(e) {
    fs_extra_1.ensureFileSync(this.intermediatesTempShareConfigFilePath), fs_extra_1.writeFileSync(this.intermediatesTempShareConfigFilePath, JSON.stringify({
      crossAppSharedConfig: e
    }));
  }
}
