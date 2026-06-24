/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-startup-config.js
 */
import * as path from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as fse from "fs-extra";
import * as common_util_js_1 from "../common/common-util";
import * as module_option_path_info_js_1 from "../common/module-option-path-info";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as module_type_enum_js_1 from "../enum/module-type-enum";
import * as collect_duplicate_har_dependencies_js_1 from "../har-dedepulicate/collect_duplicate_har_dependencies";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
import * as dependency_util_js_1 from "../utils/dependency-util";
import * as hsp_target_utils_js_1 from "../utils/hsp-target-utils";
import * as inject_util_js_1 from "../utils/inject-util";
import * as json_util_js_1 from "../utils/json-util";
import * as res_model_loader_js_1 from "../utils/loader/file/res-model-loader";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as ohmurl_utils_js_1 from "../utils/ohmurl-utils";
import * as validate_util_js_1 from "../utils/validate/validate-util";
import * as task_names_js_1 from "./common/task-names";
import * as deduplicate_util_js_1 from "./har-deduplicate/deduplicate-util";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
var __decorate = this && this.__decorate || function (t, e, s, r) {
  var o,
    i = arguments.length,
    a = i < 3 ? e : null === r ? r = Object.getOwnPropertyDescriptor(e, s) : r;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) a = Reflect.decorate(t, e, s, r);else for (var n = t.length - 1; n >= 0; n--) (o = t[n]) && (a = (i < 3 ? o(a) : i > 3 ? o(e, s, a) : o(e, s)) || a);
  return i > 3 && a && Object.defineProperty(e, s, a), a;
};
export class ProcessStartupConfig extends ohos_hap_task_js_1.OhosHapTask {
  constructor(t) {
    super(t, task_names_js_1.TaskNames.Task.PROCESS_STARTUP_CONFIG), this._log = ohos_logger_js_1.OhosLogger.getLogger("hvigor-processStartupConfig");
    const e = t.getTargetData().getModuleSourceSetModel().getModuleTargetRes();
    if (this.targetJsonPath = e.getJsonPath(), this.targetService.shouldDoDeduplicateTask()) if (this.moduleModel.isHapModule()) {
      const t = (0, collect_duplicate_har_dependencies_js_1.getMergedHapAllHspDependencies)();
      this.allHspDepTargets = hsp_target_utils_js_1.HspTargetUtils.calHspTargets(this.targetService, t);
    } else this.moduleModel.isHspModule() && (this.hspNeedDeduplicateHarInfo = (0, collect_duplicate_har_dependencies_js_1.getHspNeedDeduplicateHarInfoMap)(this.moduleName));
  }
  get useNormalizedOHMUrl() {
    var t, e;
    return !!(null === (e = null === (t = this.targetService.getBuildOption()) || undefined === t ? undefined : t.strictMode) || undefined === e ? undefined : e.useNormalizedOHMUrl);
  }
  get shouldCollectPreloadSystemSo() {
    const t = this.targetName === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET,
      e = inject_util_js_1.InjectUtil.isColdReload() || inject_util_js_1.InjectUtil.isPreviewProcess();
    return !(!this.moduleModel.isHapModule() || e || inject_util_js_1.InjectUtil.isLocalTest() || t) && this.isPreloadSystemSoEnabled;
  }
  declareInputs() {
    var t;
    const e = super.declareInputs();
    e.set("appStartupFileName", null !== (t = this.appStartupFileName) && undefined !== t ? t : "");
    const s = this.moduleModel.getModuleType();
    if (s === module_type_enum_js_1.ModuleType.Har && (e.set("isBundledDependencies", this.targetService.isBundledDependencies()), e.set("isByteCodeHar", this.targetService.isByteCodeHar())), s !== module_type_enum_js_1.ModuleType.Har || this.targetService.isBundledDependencies()) {
      const t = this.service.getHarDependencies();
      for (let s = 0; s < t.length; s++) {
        const r = t[s];
        if (!r.isLocal()) continue;
        const [o] = this.getLocalHarDepStartupConfig(r);
        e.set(r.getDependencyName(), null != o ? o : "");
      }
    }
    return e;
  }
  declareInputFiles() {
    const t = super.declareInputFiles();
    this.appStartupPath && fse.existsSync(this.appStartupPath) && t.addEntry(this.appStartupPath);
    if (this.moduleModel.getModuleType() !== module_type_enum_js_1.ModuleType.Har || this.targetService.isBundledDependencies()) {
      const e = this.service.getHarDependencies();
      for (let s = 0; s < e.length; s++) {
        const r = e[s];
        if (!r.isLocal()) continue;
        const [, o] = this.getLocalHarDepStartupConfig(r);
        o && fse.existsSync(o) && t.addEntry(o);
      }
    }
    return fse.existsSync(this.pathInfo.getIntermediatesPkgContextInfoPath()) && t.addEntry(this.pathInfo.getIntermediatesPkgContextInfoPath()), this.targetService.shouldDoDeduplicateTask() && this.moduleModel.isHapModule() && this.processHarDeduplicateInputFiles(t), t;
  }
  declareOutputFiles() {
    const t = super.declareOutputFiles().addEntry(this.intermediateTempStartupFilePath);
    return this.hspNeedDeduplicateHarInfo && Object.keys(this.hspNeedDeduplicateHarInfo).length > 0 && t.addEntry(this.pathInfo.getStartupDuplicateInfoPath()), t;
  }
  doTaskAction() {
    this.appStartupValidate(), this.checkAppStartupConfig();
  }
  initTaskDepends() {}
  appStartupValidate() {
    if (this.checkResReferenceConfig(common_const_js_1.CommonConst.APP_STARTUP), !this.appStartupPath || !fse.existsSync(this.appStartupPath)) return;
    let t = this.sdkInfo.getAppStartupSchema();
    const e = this.moduleModel.getModuleType();
    e === module_type_enum_js_1.ModuleType.Shared && (t = this.sdkInfo.getHspStartupSchema()), e === module_type_enum_js_1.ModuleType.Har && (t = this.sdkInfo.getHarStartupSchema()), validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.moduleName,
      filePath: this.appStartupPath,
      schemaPath: t
    });
  }
  checkAppStartupConfig() {
    const t = this.appStartupPath,
      e = this.moduleModel.getModuleType(),
      s = this.pathInfo.getIntermediatesPkgContextInfoPath(),
      r = json_util_js_1.JsonUtil.getJson5Obj(s);
    if (undefined !== t && fse.existsSync(t) && this.processModuleStartupConfig(t, r, e), e !== module_type_enum_js_1.ModuleType.Har || this.targetService.isBundledDependencies()) {
      const t = this.service.getHarDependencies();
      for (let e = 0; e < t.length; e++) {
        const s = t[e];
        s.isLocal() ? this.processLocalHarDependency(s, r) : (this.processRemoteHarDependency(s, r), this.addDependencyPreloadSystemSoJson(s));
      }
    }
    undefined !== this.startupMergedOptions ? (this.checkStartupTaskName(this.startupMergedOptions.startupTasks, "startupTask"), this.checkStartupTaskName(this.startupMergedOptions.appPreloadHintStartupTasks, "appPreloadHintStartupTask"), this.targetService.shouldDoDeduplicateTask() && this.processHarDeduplicate(), fse.ensureFileSync(this.intermediateTempStartupFilePath), fse.writeFileSync(this.intermediateTempStartupFilePath, JSON.stringify(this.startupMergedOptions))) : fse.existsSync(this.intermediateTempStartupFilePath) && fse.removeSync(this.intermediateTempStartupFilePath);
  }
  checkResReferenceConfig(t) {
    const e = res_model_loader_js_1.resModelLoader.getModuleJson(this.targetJsonPath).module[t];
    if (!e) return;
    const s = (0, common_util_js_1.parsingProfileName)(e);
    if (!s) return;
    const r = module_option_path_info_js_1.moduleOptionPathInfo.getModuleOptPath(this.targetJsonPath, t),
      o = this.targetService.getTargetResourceBaseProfilePath(`${s}.json`);
    o && fse.existsSync(o) || this._log.printErrorExit("STARTUP_CONFIG_FILE_NOT_EXIST", [this.moduleName, r], [[o]]);
  }
  processLocalHarDependency(t, e) {
    const s = dependency_manager_js_1.DependencyManager.getModuleModelByDependency(t, this.projectModel);
    if (undefined === s) return;
    const r = s.getModule().getName(),
      [o, i] = this.getLocalHarDepStartupConfig(t);
    if (!o || !i) return;
    const a = json_util_js_1.JsonUtil.getJson5Obj(i);
    undefined !== a.startupTasks && a.startupTasks.forEach(e => {
      this.checkStartupPath(i, e.srcEntry, null == s ? undefined : s.getModule().getNodeDir(), r), e.srcEntryAbsolutePath = path.resolve(null == s ? undefined : s.getModule().getNodeDir(), common_const_js_1.CommonConst.SRC_MAIN, e.srcEntry), e.moduleName = r, e.startupConfigPath = i, e.ohmurl = (0, ohmurl_utils_js_1.generateOhmurlForSrcEntry)(this.targetService, this.moduleName, t, e.srcEntry);
    }), undefined !== a.appPreloadHintStartupTasks && a.appPreloadHintStartupTasks.forEach(t => {
      var s;
      this.checkAppPreloadHintStartupTaskPath(i, t.srcEntry, r, e), t.moduleName = r, t.startupConfigPath = i;
      const o = path.basename(t.srcEntry);
      t.ohmurl = this.useNormalizedOHMUrl ? (0, ohmurl_utils_js_1.generateOhmUrlForSo)(null !== (s = e[o]) && undefined !== s ? s : {}, t.srcEntry) : (0, ohmurl_utils_js_1.generateOldOhmUrlForSo)(this.targetService, t.srcEntry, this.moduleName);
    }), undefined === this.startupMergedOptions ? this.startupMergedOptions = a : (this.startupMergedOptions.startupTasks = [...(this.startupMergedOptions.startupTasks || []), ...(a.startupTasks || [])], this.startupMergedOptions.appPreloadHintStartupTasks = [...(this.startupMergedOptions.appPreloadHintStartupTasks || []), ...(a.appPreloadHintStartupTasks || [])]);
  }
  getLocalHarDepStartupConfig(t) {
    if (undefined === dependency_manager_js_1.DependencyManager.getModuleModelByDependency(t, this.projectModel)) return [undefined, undefined];
    const e = t.getModuleJsonObj();
    if (undefined === e) return [undefined, undefined];
    const s = e.module.appStartup;
    if (undefined === s) return [undefined, undefined];
    const r = (0, common_util_js_1.parsingProfileName)(s);
    if (undefined === r) return [undefined, undefined];
    return [r, this.getLocalHarDepStartupFile(this.targetService, t, this.projectModel, r)];
  }
  getLocalHarDepStartupFile(t, e, s, r) {
    const o = (0, dependency_util_js_1.getLocalDepHarTargetSources)(t, e, s);
    for (const t of o) {
      const e = path.resolve(t, common_const_js_1.CommonConst.BASE_PROFILE, `${r}.json`);
      if (fse.existsSync(e)) return e;
    }
  }
  addDependencyPreloadSystemSoJson(t) {
    if (!this.shouldCollectPreloadSystemSo || !t.isByteCodeHarDependency()) return;
    const e = path.resolve(t.getDependencyRootPath(), build_directory_const_js_1.BuildDirConst.META_INF, build_directory_const_js_1.BuildArtifactConst.PRELOAD_SO_FILE_NAME);
    if (!fse.existsSync(e)) return;
    const s = json_util_js_1.JsonUtil.getJson5Obj(e);
    s && (this.startupMergedOptions ? this.startupMergedOptions.systemPreloadHintStartupTasks = [...(this.startupMergedOptions.systemPreloadHintStartupTasks || []), ...(s.systemPreloadHintStartupTasks || [])] : this.startupMergedOptions = s);
  }
  processRemoteHarDependency(t, e) {
    var s, r, o;
    const i = path.resolve(t.getDependencyRootPath(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN),
      a = dependency_manager_js_1.DependencyManager.getModuleObjByRemoteDependency(t),
      n = null !== (s = (0, common_util_js_1.parsingProfileName)(a.module.appStartup)) && undefined !== s ? s : undefined;
    if (!n) return;
    const l = this.getRemoteHarDepStartupFile(t, n);
    if (!l) return;
    const u = json_util_js_1.JsonUtil.getJson5Obj(l);
    null === (r = u.startupTasks) || undefined === r || r.forEach(e => {
      e.srcEntryAbsolutePath = path.resolve(i, e.srcEntry), e.moduleName = a.module.name, e.startupConfigPath = l, e.ohmurl = (0, ohmurl_utils_js_1.generateOhmurlForSrcEntry)(this.targetService, this.moduleName, t, e.srcEntry);
    }), null === (o = u.appPreloadHintStartupTasks) || undefined === o || o.forEach(s => {
      var r;
      s.moduleName = a.module.name, s.startupConfigPath = l, s.isRemoteByteCodeHar = t.isByteCodeHarDependency();
      const o = path.basename(s.srcEntry);
      s.ohmurl = this.useNormalizedOHMUrl ? (0, ohmurl_utils_js_1.generateOhmUrlForSo)(null !== (r = e[o]) && undefined !== r ? r : {}, s.srcEntry) : (0, ohmurl_utils_js_1.generateOldOhmUrlForSo)(this.targetService, s.srcEntry, this.moduleName);
    }), undefined === this.startupMergedOptions ? this.startupMergedOptions = u : (this.startupMergedOptions.startupTasks = [...(this.startupMergedOptions.startupTasks || []), ...(u.startupTasks || [])], this.startupMergedOptions.appPreloadHintStartupTasks = [...(this.startupMergedOptions.appPreloadHintStartupTasks || []), ...(u.appPreloadHintStartupTasks || [])]);
  }
  getRemoteHarDepStartupFile(t, e) {
    const s = (0, dependency_util_js_1.getRemoteDepHarTargetSources)(t);
    for (const t of s) {
      const s = path.resolve(t, common_const_js_1.CommonConst.BASE_PROFILE, `${e}.json`);
      if (fse.existsSync(s)) return s;
    }
  }
  processModuleStartupConfig(t, e, s) {
    const r = json_util_js_1.JsonUtil.getJson5Obj(t);
    undefined !== (null == r ? undefined : r.startupTasks) && r.startupTasks.forEach(r => {
      var o;
      this.checkStartupPath(t, r.srcEntry, this.moduleModel.getModule().getNodeDir(), this.moduleName);
      const i = path.resolve(this.module.getNodeDir(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN);
      r.srcEntryAbsolutePath = path.resolve(i, r.srcEntry), r.moduleName = this.moduleName, r.startupConfigPath = t, (this.targetService.isByteCodeHar() || s !== module_type_enum_js_1.ModuleType.Har) && (r.ohmurl = this.useNormalizedOHMUrl ? (0, ohmurl_utils_js_1.generateOhmUrlForSourceFile)(null !== (o = e[this.packageJsonObj.name]) && undefined !== o ? o : {}, path.join(build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, r.srcEntry)) : (0, ohmurl_utils_js_1.getOldOhmUrlForSrcEntry)(this.targetService, r.srcEntry, this.moduleName));
    }), undefined !== (null == r ? undefined : r.appPreloadHintStartupTasks) && r.appPreloadHintStartupTasks.forEach(r => {
      var o;
      if (this.checkAppPreloadHintStartupTaskPath(t, r.srcEntry, this.moduleName, e), r.moduleName = this.moduleName, r.startupConfigPath = t, this.targetService.isByteCodeHar() || s !== module_type_enum_js_1.ModuleType.Har) {
        const t = path.basename(r.srcEntry);
        r.ohmurl = this.useNormalizedOHMUrl ? (0, ohmurl_utils_js_1.generateOhmUrlForSo)(null !== (o = e[t]) && undefined !== o ? o : {}, r.srcEntry) : (0, ohmurl_utils_js_1.generateOldOhmUrlForSo)(this.targetService, r.srcEntry, this.moduleName);
      }
    }), undefined !== (null == r ? undefined : r.configEntry) && this.checkStartupPath(t, r.configEntry, this.moduleModel.getModule().getNodeDir(), this.moduleName), undefined !== r && (this.startupMergedOptions = r);
  }
  checkStartupPath(t, e, s, r) {
    if (undefined === e) return;
    const o = path.resolve(s, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, e);
    path.isAbsolute(e) && this._log.printErrorExit("STARTUP_SRCENTRY_IS_ABSOLUTE", [e, r, t], []), common_const_js_1.ValidateRegExp.STARTUP_TASK_SRC_ENTRY_REG_EXP.test(path.extname(o)) || this._log.printErrorExit("STARTUP_SRCENTRY_SUFFIX", [e, r, t], []), o.startsWith(s) || this._log.printErrorExit("STARTUP_SRCENTRY_MODULE_FILE", [e, r, t], []), fse.existsSync(o) || this._log.printErrorExit("STARTUP_SRCENTRY_FILE_EXIST", [e, r, t], []);
  }
  checkAppPreloadHintStartupTaskPath(t, e, s, r) {
    if (common_const_js_1.ValidateRegExp.PRELOAD_STARTUP_TASK_SRC_ENTRY_REG_EXP.test(e) || this._log.printErrorExit("APP_PRELOAD_STARTUP_SRCENTRY_SUFFIX", [e, s, t], []), this.useNormalizedOHMUrl && r) {
      Object.keys(r).includes(path.basename(e)) || this._log.printErrorExit("STARTUP_SO_FILE_EXIST", [e, s, t], [[e]]);
    }
  }
  checkStartupTaskName(t, e) {
    if (!t || t.length <= 1) return;
    const s = new Set();
    for (let r = 0; r < t.length; r++) {
      const o = t[r].name;
      s.has(o) ? this._log.printErrorExit("STARTUP_TASK_NAME_UNIQUE", [o, e, t[r].moduleName], [[e, t[r].moduleName, o]]) : s.add(o);
    }
  }
  processHarDeduplicateInputFiles(t) {
    this.allHspDepTargets && this.allHspDepTargets.forEach(e => {
      const s = e.getPathInfo().getStartupDuplicateInfoPath();
      fse.existsSync(s) && t.addEntry(s);
    });
  }
  processHarDeduplicate() {
    this.moduleModel.isHspModule() ? this.collectDuplicateHarStartup() : this.moduleModel.isHapModule() && this.addDuplicateHarStartups();
  }
  collectDuplicateHarStartup() {
    if (!this.startupMergedOptions) return;
    if (!this.hspNeedDeduplicateHarInfo) return;
    const t = Object.keys(this.hspNeedDeduplicateHarInfo).map(t => path.normalize(t)),
      e = e => {
        const s = e;
        return undefined !== t.find(t => path.normalize(s.startupConfigPath).startsWith(t));
      },
      s = (0, deduplicate_util_js_1.deduplicateArray)(this.startupMergedOptions.startupTasks || [], e),
      r = (0, deduplicate_util_js_1.deduplicateArray)(this.startupMergedOptions.appPreloadHintStartupTasks || [], e);
    if (s.length > 0 || r.length > 0) {
      const t = {
        appPreloadHintStartupTasks: r,
        startupTasks: s
      };
      this._log.debug(`HarDeduplicate hsp: ${this.moduleName} collected startupOptions ${t}`), fse.outputJSONSync(this.pathInfo.getStartupDuplicateInfoPath(), t);
    }
  }
  addDuplicateHarStartups() {
    this.allHspDepTargets && this.allHspDepTargets.forEach(t => {
      var e, s, r, o;
      const i = t.getPathInfo().getStartupDuplicateInfoPath();
      if (!fse.existsSync(i)) return;
      const a = fse.readJSONSync(i);
      a && (this._log.debug(`HarDeduplicate hap merge startupOptions ${a}`), a.startupTasks && (null === (e = this.startupMergedOptions) || undefined === e ? undefined : e.startupTasks) && (0, deduplicate_util_js_1.mergeTwoArray)(a.startupTasks, null === (s = this.startupMergedOptions) || undefined === s ? undefined : s.startupTasks, (t, e) => t.name === e.name), a.appPreloadHintStartupTasks && (null === (r = this.startupMergedOptions) || undefined === r ? undefined : r.appPreloadHintStartupTasks) && (0, deduplicate_util_js_1.mergeTwoArray)(a.appPreloadHintStartupTasks, null === (o = this.startupMergedOptions) || undefined === o ? undefined : o.appPreloadHintStartupTasks, (t, e) => t.name === e.name));
    });
  }
}
__decorate([(0, hvigor_1.Input)()], ProcessStartupConfig.prototype, "useNormalizedOHMUrl", null), __decorate([(0, hvigor_1.Input)()], ProcessStartupConfig.prototype, "shouldCollectPreloadSystemSo", null);
