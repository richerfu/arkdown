/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/make-project-pack-info.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_2 from "@ohos/hvigor";
import * as fse from "fs-extra";
import path_1 from "path";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as task_names_js_1 from "./common/task-names";
import * as ohos_app_task_js_1 from "./task/ohos-app-task";
import * as json_util_js_1 from "../utils/json-util";
import * as module_task_service_js_1 from "./service/module-task-service";
import * as task_util_1 from "@ohos/hvigor/src/base/util/task-util";
import * as remote_hsp_utils_js_1 from "../utils/remote-hsp-utils";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
import * as inject_util_js_1 from "../utils/inject-util";
import * as hvigor_common_1 from "@ohos/hvigor-common";
var __decorate = this && this.__decorate || function (e, t, s, o) {
  var a,
    i = arguments.length,
    r = i < 3 ? t : null === o ? o = Object.getOwnPropertyDescriptor(t, s) : o;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) r = Reflect.decorate(e, t, s, o);else for (var n = e.length - 1; n >= 0; n--) (a = e[n]) && (r = (i < 3 ? a(r) : i > 3 ? a(t, s, r) : a(t, s)) || r);
  return i > 3 && r && Object.defineProperty(t, s, r), r;
};
var CommonHookTask = task_names_js_1.TaskNames.CommonHookTask;
const Task = task_names_js_1.TaskNames.Task;
export class MakeProjectPackInfo extends ohos_app_task_js_1.OhosAppTask {
  get alignDeviceTypes() {
    return this.service.getProjectModel().isFaMode() ? [] : inject_util_js_1.InjectUtil.getAlignDeviceTypes();
  }
  declareInputs() {
    const e = new Map();
    return e.set("allRemoteHspPathMap", JSON.stringify([...this.allRemoteHspPathMap])), e;
  }
  declareInputFiles() {
    const e = new hvigor_2.FileSet();
    for (const t of this.hapPackInfoPathMap.keys()) e.addEntry(t);
    return e;
  }
  declareOutputFiles() {
    const e = path_1.resolve(this.service.getPathInfo().getProjectOutputPath(), build_directory_const_js_1.BuildArtifactConst.PACK_INFO);
    return new hvigor_2.FileSet().addEntry(e);
  }
  initTaskDepends() {
    hvigor_1.hvigor.nodesEvaluatedInternal(() => {
      const e = this.service.getProjectModel().getSubModuleModels();
      this._log.debug(`project has submodules:${[...e.keys()]}`), e.forEach(e => {
        const t = e,
          s = this.isNeedPackageHap(t);
        t.isHapModule() && s ? this.dependsOn(CommonHookTask.ASSEMBLE_HAP.name, e.getName()) : this._log.debug(`module:${e.getName()} no need to execute packageHap`), t.isHspModule() && s && this.dependsOn(CommonHookTask.ASSEMBLE_HSP.name, e.getName());
      });
    });
  }
  isAlignDeviceTypesContained(e) {
    const t = this.alignDeviceTypes;
    return !t.length || t.every(t => !!(null == e ? undefined : e.includes(t)));
  }
  normalizeDeviceTypes(e) {
    const t = this.alignDeviceTypes;
    t.length && e && (e.deviceType = [...t]);
  }
  traverseNormalizeDeviceTypes(e) {
    return null == e || e.forEach(e => this.normalizeDeviceTypes(e)), e;
  }
  isNeedPackageHap(e) {
    if ((0, task_util_1.getAlignTarget)()) return true;
    const t = this.service.getTargetProduct().name,
      s = this.service.getProjectModel().getModuleProfileOpt(e.getName()),
      o = null == s ? undefined : s.targets;
    return undefined !== o && o.some(e => {
      const s = e.name;
      let o = e.applyToProducts;
      return undefined === o && (o = ["default"]), s !== common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET && o.includes(t);
    });
  }
  constructor(e, t) {
    super(e, t, Task.MAKE_PROJECT_PACK_INFO), this._log = ohos_logger_js_1.OhosLogger.getLogger("hvigor-make-project-info"), this.moduleSet = new Set(), this.hapPackInfoPathMap = new Map(), this.allRemoteHspPathMap = new Map(), this.remoteHspModuleSet = new Set(), this._projectPackInfoObj = {
      summary: {
        app: {
          bundleName: "bundleName",
          version: {
            name: "name",
            code: 0
          }
        },
        modules: []
      },
      packages: []
    };
  }
  async beforeAlwaysAction() {
    const e = this.service.getProjectModel().getRemoteHspPath();
    this.service.getProductDataMap().forEach(t => {
      this.handleModule(t, e);
    });
  }
  handleModule(e, t) {
    for (const s of e) {
      const e = s.getModuleModel().getName(),
        o = s.getRelatedEntryModules(),
        a = s.getModuleModel().getModuleType();
      if (module_task_service_js_1.ModuleTaskService.checkEntryModules(a, o)) for (const t of o) {
        const o = path_1.resolve(s.getPathInfo().getModuleBuildOutputPath(), t, build_directory_const_js_1.BuildArtifactConst.PACK_INFO);
        this.hapPackInfoPathMap.set(o, e);
      } else {
        const t = path_1.resolve(s.getPathInfo().getModuleBuildOutputPath(), build_directory_const_js_1.BuildArtifactConst.PACK_INFO);
        this.hapPackInfoPathMap.set(t, e);
      }
      const i = s.getModuleModel(),
        r = i.getRemoteHspPath(),
        n = new dependency_manager_js_1.DependencyManager(this.service.getProjectModel().isFaMode(), i, this.service.getProjectModel()),
        {
          npmDependencies: c
        } = n.collectAllDependencies();
      this.remoteHspModuleSet.has(i.getName()) || ((0, remote_hsp_utils_js_1.initSignRemoteHspMap)(r, this.allRemoteHspPathMap, c), (0, remote_hsp_utils_js_1.initSignRemoteHspMap)(t, this.allRemoteHspPathMap, c), this.remoteHspModuleSet.add(i.getName()));
    }
  }
  async doTaskAction() {
    this.hapPackInfoPathMap.forEach((e, t) => {
      this.mergeHapPackInfo(t, e);
    });
    const e = [...this.allRemoteHspPathMap.values()];
    for (const {
      hspPath: t,
      hspFileName: s
    } of e) {
      const e = await (0, hvigor_common_1.readFileInZIP)(t, [build_directory_const_js_1.BuildArtifactConst.PACK_INFO, common_const_js_1.CommonConst.MODULE_JSON]),
        o = e.get(build_directory_const_js_1.BuildArtifactConst.PACK_INFO),
        a = e.get(common_const_js_1.CommonConst.MODULE_JSON);
      o && a && this.mergeRemoteHspPackInfo(JSON.parse(o), JSON.parse(a).module.name, s);
    }
    this._log.anonymizeDebug("Project Pack Info:", this._projectPackInfoObj), fse.outputJSONSync(path_1.resolve(this.service.getPathInfo().getProjectOutputPath(), build_directory_const_js_1.BuildArtifactConst.PACK_INFO), this._projectPackInfoObj, {
      spaces: "\t"
    }), this.moduleSet.clear();
  }
  mergeHapPackInfo(e, t) {
    if (fse.existsSync(e)) {
      const s = json_util_js_1.JsonUtil.getJson5Obj(e),
        o = s.summary.modules[0];
      if (!this.isAlignDeviceTypesContained(o.deviceType)) return;
      s.summary.app.bundleName && (this._projectPackInfoObj.summary.app = s.summary.app), this.moduleSet.has(t) || (this.normalizeDeviceTypes(o), this._projectPackInfoObj.summary.modules.push(o), this.moduleSet.add(t)), this._projectPackInfoObj.packages = this._projectPackInfoObj.packages.concat(this.traverseNormalizeDeviceTypes(s.packages));
    }
  }
  mergeRemoteHspPackInfo(e, t, s) {
    const o = e.summary.modules[0];
    this.isAlignDeviceTypesContained(o.deviceType) && (e.packages[0].name = this.removeHspExtension(s), this.moduleSet.has(t) || (this.normalizeDeviceTypes(o), this._projectPackInfoObj.summary.modules.push(o), this.moduleSet.add(t)), this._projectPackInfoObj.packages = this._projectPackInfoObj.packages.concat(this.traverseNormalizeDeviceTypes(e.packages)));
  }
  removeHspExtension(e) {
    const t = build_directory_const_js_1.BuildArtifactExtension.DOT_HSP;
    return e.endsWith(t) ? e.slice(0, -t.length) : e;
  }
}
__decorate([(0, hvigor_1.Input)()], MakeProjectPackInfo.prototype, "alignDeviceTypes", null);
