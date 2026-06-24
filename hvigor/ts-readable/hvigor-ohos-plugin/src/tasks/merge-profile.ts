/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/merge-profile.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as fs from "fs-extra";
import * as fs_extra_1 from "fs-extra";
import * as project_ohos_config_manager_js_1 from "../common/global/project-ohos-config-manager";
import * as module_dependency_util_js_1 from "../common/module-dependency-util";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as build_mode_const_js_1 from "../const/build-mode-const";
import * as common_const_js_1 from "../const/common-const";
import * as sdk_const_js_1 from "../const/sdk-const";
import * as module_type_enum_js_1 from "../enum/module-type-enum";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as abstract_merge_profile_js_1 from "./abstract-merge-profile";
import * as task_names_js_1 from "./common/task-names";
import * as pre_build_js_1 from "./pre-build";
import * as array_util_js_1 from "../utils/array-util";
import * as inject_util_js_1 from "../utils/inject-util";
import * as har_target_util_js_1 from "../utils/har-target-util";
import * as json_util_js_1 from "../utils/json-util";
import * as hvigor_config_loader_js_1 from "@ohos/hvigor/src/common/util/hvigor-config-loader";
import * as dependency_interface_js_1 from "../project/dependency/core/dependency-interface";
import * as hsp_target_utils_js_1 from "../utils/hsp-target-utils";
import * as ohos_test_util_js_1 from "../utils/ohos-test-util";
import * as task_util_js_1 from "../utils/task-util";
var __decorate = this && this.__decorate || function (e, t, s, i) {
  var o,
    r = arguments.length,
    n = r < 3 ? t : null === i ? i = Object.getOwnPropertyDescriptor(t, s) : i;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) n = Reflect.decorate(e, t, s, i);else for (var a = e.length - 1; a >= 0; a--) (o = e[a]) && (n = (r < 3 ? o(n) : r > 3 ? o(t, s, n) : o(t, s)) || n);
  return r > 3 && n && Object.defineProperty(t, s, n), n;
};
var Task = task_names_js_1.TaskNames.Task;
const _log = ohos_logger_js_1.OhosLogger.getLogger("hvigor-merge-profile");
export class MergeProfile extends abstract_merge_profile_js_1.AbstractMergeProfile {
  constructor(e) {
    var t, s;
    super(e, Task.MERGE_PROFILE), this._ohLibs = [], this._isHarModule = this.service.getModuleModel().isHarModule(), this._multiProjects = this._projectModel.getProfileOpt().app.multiProjects || false, this._appRes = this._projectModel.getAppRes();
    const i = this.service.getModuleModel().getSourceSetByTargetName(this._targetName);
    this._moduleTargetRes = i.getModuleTargetRes(), this._mergedModuleJson = this._pathInfo.getIntermediatesMergeProfile(), this._moduleBuildAbilities = null === (s = null === (t = (0, array_util_js_1.getElementFromArr)(this.moduleModel.getProfileOpt().targets, this.targetName)) || undefined === t ? undefined : t.source) || undefined === s ? undefined : s.abilities;
  }
  declareInputs() {
    var e, t, s, i, o;
    const r = this.targetData.getProduct();
    return super.declareInputs().set(abstract_merge_profile_js_1.INPUT_KEY.isHarModule, this._isHarModule).set(abstract_merge_profile_js_1.INPUT_KEY.multiProjects, this._multiProjects).set(abstract_merge_profile_js_1.INPUT_KEY.asanEnable, this.asanEnable).set(abstract_merge_profile_js_1.INPUT_KEY.hwasanEnable, this.hwAsanEnable).set(abstract_merge_profile_js_1.INPUT_KEY.ubsanEnable, this.ubsanEnable).set(abstract_merge_profile_js_1.INPUT_KEY.isDebug, this.targetService.isDebug()).set(abstract_merge_profile_js_1.INPUT_KEY.buildProfileAbilities, JSON.stringify(this._moduleBuildAbilities)).set(abstract_merge_profile_js_1.INPUT_KEY.projectConfigAppOpt, JSON.stringify(null === (e = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === e ? undefined : e.appOpt)).set(abstract_merge_profile_js_1.INPUT_KEY.moduleJsonOpt, JSON.stringify(this._moduleTargetRes.getModuleJsonOpt())).set(abstract_merge_profile_js_1.INPUT_KEY.appJsonOpt, JSON.stringify(this._appRes.getAppResOpt())).set("integratedHsp", !!(null === (t = this.targetService.getBuildOption().arkOptions) || undefined === t ? undefined : t.integratedHsp)).set("removePermissions", JSON.stringify(null === (s = this.targetService.getBuildOption()) || undefined === s ? undefined : s.removePermissions)).set("isBundledDependencies", this.isBundledDependencies).set("packageName", null !== (i = this.packageJsonObj.name) && undefined !== i ? i : "").set("ohLibs", this._ohLibs).set("buildVersion", null !== (o = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.BUILD_VERSION)) && undefined !== o ? o : "").set("productBuildOpt", JSON.stringify(r));
  }
  get includeAppScopeRes() {
    var e;
    return null === (e = this.targetService.getBuildOption().resOptions) || undefined === e ? undefined : e.includeAppScopeRes;
  }
  declareInputFiles() {
    const e = new hvigor_1.FileSet().addEntry(this._appRes.getJsonPath()).addEntry(this.projectModel.getProfilePath()).addEntry(this._moduleTargetRes.getJsonPath());
    return this._ohLibs.forEach(t => {
      fs.existsSync(t) && e.addEntry(t);
    }), e;
  }
  declareOutputFiles() {
    var e;
    const t = new hvigor_1.FileSet().addEntry(this._mergedModuleJson);
    return (this.moduleModel.isHspModule() || this.moduleModel.isHarModule()) && (null === (e = this._moduleTargetRes.getModuleJsonOpt().module.requestPermissions) || undefined === e ? undefined : e.length) && (0, task_util_js_1.getBuildModeName)() === build_mode_const_js_1.BuildModeConst.TEST && t.addEntry(this._pathInfo.getIntermediatesOhosTestResourceStr(), {
      isDirectory: true
    }), t;
  }
  get isBundledDependencies() {
    return this.targetService.isBundledDependencies();
  }
  async beforeAlwaysAction() {
    await super.beforeAlwaysAction(), this._ohLibs = this.mergeDependsLibs(common_const_js_1.CommonConst.MODULE_JSON);
  }
  initTaskDepends() {
    this.declareDepends(pre_build_js_1.PreBuild.name);
  }
  initHarModuleDepends() {
    har_target_util_js_1.HarTargetUtil.calDepHarTargets(this.targetService).forEach((e, t) => this.declareDepends(`${e}@${Task.MERGE_PROFILE.name}`, t));
  }
  async doTaskAction() {
    var e, t, s;
    const i = this.computeAppJsonOpt();
    this.mergeDslConfig(i), this.mergeAppEnvironmentConfig(i);
    const o = (0, wdk_1.cloneDeep)(this._moduleTargetRes.getModuleJsonOpt());
    this.targetName === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET && (0, ohos_test_util_js_1.isTemplatesAfterSourceCodeMigration)(path_1.resolve(this._moduleTargetRes.getResourcePath(), "..")) && (o.module = {
      ...(0, ohos_test_util_js_1.getOhosTestAbilityConfig)(),
      ...o.module
    }), Object.assign(o.module, {
      packageName: this.packageJsonObj.name
    });
    const r = this.mergeBuildProfileAbilities(o),
      n = this.moduleModel.isHspModule() || this.moduleModel.isHapModule() ? this.getRemovePermissionSet(r) : new Set(),
      a = this.mergeAllHarModuleOpt(r);
    (null === (e = null == a ? undefined : a.module) || undefined === e ? undefined : e.requestPermissions) && n.size > 0 && (a.module.requestPermissions = null === (t = a.module.requestPermissions) || undefined === t ? undefined : t.filter(e => !(null == n ? undefined : n.has(e.name)))), fs_extra_1.default.existsSync(this.pathInfo.getResourceStr()) && fs.rmSync(this.pathInfo.getResourceStr(), {
      recursive: true,
      force: true
    });
    const l = a.module.requestPermissions;
    this.processResourceStr(l), null === (s = o.module.proxyData) || undefined === s || s.forEach(e => {
      if (this._bundleName) {
        const t = e.uri.split("/")[2];
        this._appRes.getAppResOpt().app.bundleName === t && (e.uri = e.uri.replace(t, this._bundleName));
      }
    });
    const d = {
      ...i,
      ...a
    };
    this.customizeModuleOpt(d, this.targetData.getProduct()), !this.targetService.isIntegratedHsp() || inject_util_js_1.InjectUtil.isPreviewProcess() || inject_util_js_1.InjectUtil.isLocalTest() || (d.app.bundleName = ""), this.moduleModel.isHspModule() && false === this.includeAppScopeRes && (d.app.icon = "", d.app.label = "");
    hvigor_config_loader_js_1.HvigorConfigLoader.getInstance().getPropertiesConfigValue(common_const_js_1.HvigorConfigConst.OHOS_PACK_COMPRESS_LEVEL) && (d.module.compressNativeLibs = true), await this.clearArkTSCacheByModuleJSONChange(d), fs.outputJSONSync(this._mergedModuleJson, d, {
      spaces: "\t"
    });
  }
  async clearArkTSCacheByModuleJSONChange(e) {
    if (!fs_extra_1.default.existsSync(this._mergedModuleJson)) return;
    const t = json_util_js_1.JsonUtil.getJson5Obj(this._mergedModuleJson),
      s = path_1.resolve(this.targetData.getPathInfo().getModuleBuildCachePath()),
      i = this.moduleModel.isOutModule() && t.app.bundleName !== e.app.bundleName,
      o = t.app.compileSdkVersion && this.sdkInfo.getToolchainsComponentVersion() && t.app.compileSdkVersion !== this.sdkInfo.getToolchainsComponentVersion();
    (i || o) && (_log.debug("Clean the ArkTS cache due to bundleName or SDK version is changed."), await fs_extra_1.default.emptydir(s));
  }
  computeAppJsonOpt() {
    const e = (0, wdk_1.cloneDeep)(this._appRes.getAppResOpt());
    if (undefined !== e.app.multiProjects && (_log.warn(`Field 'multiProjects' should be configured in project's build-profile.json5. The value of this field configured in app.json5 will be ignored. Please configure this field in ${this._projectModel.getProfilePath()}.`), delete e.app.multiProjects), this._isHarModule) {
      const t = {
        app: {
          bundleName: e.app.bundleName,
          debug: e.app.debug,
          versionCode: e.app.versionCode,
          versionName: e.app.versionName,
          minCompatibleVersionCode: e.app.minCompatibleVersionCode,
          minAPIVersion: e.app.minAPIVersion,
          targetAPIVersion: e.app.targetAPIVersion,
          apiReleaseType: e.app.apiReleaseType,
          targetMinorAPIVersion: e.app.targetMinorAPIVersion,
          targetPatchAPIVersion: e.app.targetPatchAPIVersion
        }
      };
      this.targetService.getTargetData().getTargetName() === common_const_js_1.CommonConst.OHOS_TEST && Object.assign(t.app, {
        icon: e.app.icon,
        label: e.app.label,
        vendor: e.app.vendor,
        bundleType: e.app.bundleType
      }), e.app = t.app;
    }
    return this.asanEnable && (e.app.asanEnabled = this.asanEnable), this.tsanEnable && (e.app.tsanEnabled = this.tsanEnable), this.hwAsanEnable && (e.app.hwasanEnabled = this.hwAsanEnable), this.ubsanEnable && (e.app.ubsanEnabled = this.ubsanEnable), e;
  }
  mergeAppEnvironmentConfig(e) {
    var t;
    const s = this.parseToAppEnvironments(this.appEnvironments);
    if (undefined !== e.app.appEnvironments && 0 !== e.app.appEnvironments.length) {
      _log.debug("Change app appEnvironment");
      const i = new Map();
      s.forEach(e => {
        i.set(e.name, e.value);
      }), null === (t = e.app.appEnvironments) || undefined === t || t.forEach(e => {
        i.set(e.name, e.value);
      }), e.app.appEnvironments = Array.from(i, ([e, t]) => ({
        name: e,
        value: t
      }));
    } else _log.debug("Use cli appEnvironment"), e.app.appEnvironments = s;
  }
  processResourceStr(e) {
    module_dependency_util_js_1.moduleDependencyUtil.getPureHar(this.service, this.moduleModel, this.projectModel, this.isFaMode).has(this.moduleName) || (this.moduleModel.isHspModule() || this.moduleModel.isHarModule() ? (null == e ? undefined : e.length) && this.generateResourceStrByPermissions(e) : this.initResourceStr());
  }
  generateResourceStrByPermissions(e) {
    const t = [];
    e.forEach(e => {
      e.reason && !t.includes(e.reason) && t.push(e.reason.replace("$string:", ""));
    }), t.length && (this.copyStringToResourceStr(), this.initResourceStr(), this.filterResStr(this.pathInfo.getResourceStr(), t));
  }
  copyStringToResourceStr() {
    const e = this._moduleTargetRes.getResourcePath();
    fs_extra_1.default.existsSync(e) && fs_extra_1.default.readdirSync(e).forEach(e => {
      if ("rawfile" === e || "resfile" === e) return;
      const t = path_1.resolve(this._moduleTargetRes.getResourcePath(), e, "element", "string.json");
      if (!fs_extra_1.default.existsSync(t)) return;
      const s = path_1.resolve(this.pathInfo.getResourceStr(), e, "element", "string.json");
      fs_extra_1.default.ensureDirSync(path_1.dirname(s)), fs_extra_1.default.copySync(t, s);
    });
  }
  filterResStr(e, t) {
    fs_extra_1.default.existsSync(e) && fs_extra_1.default.readdirSync(e).forEach(s => {
      if (!("rawfile" !== s && "resfile" !== s)) return;
      const i = path_1.resolve(e, s, "element", "string.json"),
        o = {
          string: []
        };
      if (fs_extra_1.default.existsSync(i)) {
        if ((0, fs_extra_1.readJSONSync)(i).string.forEach(e => {
          t.includes(e.name) && o.string.push(e);
        }), 0 === o.string.length) return void fs_extra_1.default.removeSync(path_1.resolve(e, s));
        (0, task_util_js_1.getBuildModeName)() === build_mode_const_js_1.BuildModeConst.TEST && fs_extra_1.default.outputFileSync(path_1.resolve(this._pathInfo.getIntermediatesOhosTestResourceStr(), s, "element", "string.json"), JSON.stringify(o)), fs_extra_1.default.outputFileSync(path_1.resolve(e, s, "element", "string.json"), JSON.stringify(o));
      }
    });
  }
  mergeDslConfig(e) {
    this._bundleName && (e.app.bundleName = this._bundleName, _log.debug((0, hvigor_1.replaceBundleName)(`Change app bundleName with '${this._bundleName}'.`, this._bundleName))), this._multiProjects && (e.app.multiProjects = this._multiProjects, _log.debug(`Change app multiProjects with '${this._multiProjects}'.`)), this._vendor && (e.app.vendor = this._vendor, _log.debug(`Change app vendor with '${this._vendor}'.`)), e.app.apiReleaseType = this.sdkInfo.getReleaseType(), _log.debug(`Change app api release type with '${this.sdkInfo.getReleaseType()}'`), this.resolveApi(e.app);
  }
  resolveApi(e) {
    var t, s, i, o, r, n, a;
    const l = this.targetData.getApiMeta(),
      d = !![l.compileSdkVersion.version, l.compatibleSdkVersion.version, null !== (s = null === (t = l.targetSdkVersion) || undefined === t ? undefined : t.version) && undefined !== s ? s : l.compileSdkVersion.version].find(e => e <= sdk_const_js_1.ApiVersion.API_VERSION_9),
      u = null !== (i = l.targetSdkVersion) && undefined !== i ? i : l.compileSdkVersion,
      _ = this.targetData.isHarmonyOS() ? this.apiTransform(u) : u.version,
      p = this.targetData.isHarmonyOS() ? this.apiTransform(l.compatibleSdkVersion) : l.compatibleSdkVersion.version;
    e.compileSdkVersion = this.sdkInfo.getToolchainsComponentVersion(), _log.debug(`Change app compile API version with '${this.sdkInfo.getToolchainsComponentVersion()}'`), e.targetAPIVersion = d ? u.version : _, _log.debug(`Change app target API version with '${e.targetAPIVersion}'`), e.minAPIVersion = d ? l.compatibleSdkVersion.version : p, _log.debug(`Change app minimum API version with '${e.minAPIVersion}'`), e.compileSdkType = this.targetData.isHarmonyOS() ? "HarmonyOS" : "OpenHarmony", e.targetMinorAPIVersion = null !== (r = null === (o = l.targetSdkVersion) || undefined === o ? undefined : o.minor) && undefined !== r ? r : 0, e.targetPatchAPIVersion = null !== (a = null === (n = l.targetSdkVersion) || undefined === n ? undefined : n.patch) && undefined !== a ? a : 0;
  }
  getRemovePermissionSet(e) {
    var t, s, i;
    const o = null !== (s = null === (t = this.targetService.getBuildOption()) || undefined === t ? undefined : t.removePermissions) && undefined !== s ? s : [],
      r = new Set(o.map(e => e.name));
    if ((null == r ? undefined : r.size) > 0 && undefined !== (null === (i = null == e ? undefined : e.module) || undefined === i ? undefined : i.requestPermissions)) {
      const t = [];
      e.module.requestPermissions.forEach(e => {
        r.has(e.name) && (t.push(e.name), r.delete(e.name));
      }), t.length > 0 && _log.warn(`The permissions ${t.join(", ")} being removed in the removePermission configuration are duplicated with those in the module.json files of the hap/hsp source code modules. These permissions belong to the current module and should not be removed`);
    }
    return r;
  }
  mergeAllHarModuleOpt(e) {
    if (this._isHarModule) return this.mergeBundledDependenciesHar(e), e;
    const t = this.transformJson2Opts();
    let s = e.module.metadata,
      i = e.module.requestPermissions;
    for (let o = 0; o < t.length; o++) {
      const r = t[o];
      null !== r && (this.checkHarApiStatus(r), r.module.type !== module_type_enum_js_1.ModuleType.Har && checkHarDeviceTypes(e, r, this.targetService), s = this.mergeArrayOptions(s, r.module.metadata, r.module.name), i = this.mergeHarRequestPermission(i, r));
    }
    return 0 !== (null == s ? undefined : s.length) && (e.module.metadata = s), 0 !== (null == i ? undefined : i.length) && (e.module.requestPermissions = i), e;
  }
  mergeBundledDependenciesHar(e) {
    var t, s;
    if (!this.isBundledDependencies) return;
    const i = null !== (t = e.module.metadata) && undefined !== t ? t : [],
      o = null !== (s = e.module.requestPermissions) && undefined !== s ? s : [];
    this.service.getHarDependencies().forEach(e => {
      if (e.isByteCodeHarDependency()) return;
      const t = path_1.resolve(e.getDependencyRootPath(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, e.isLocal() ? common_const_js_1.CommonConst.MODULE_JSON5 : common_const_js_1.CommonConst.MODULE_JSON),
        s = json_util_js_1.JsonUtil.getJson5Obj(t),
        r = s.module.metadata;
      this.deDuplicateArr(r, i);
      const n = s.module.requestPermissions;
      this.deDuplicateArr(n, o);
    }), Object.assign(e.module, {
      metadata: i
    }), Object.assign(e.module, {
      requestPermissions: o
    });
  }
  deDuplicateArr(e, t) {
    null == e || e.forEach(e => {
      t.some(t => t.name === e.name) || t.push(e);
    });
  }
  mergeDependsLibs(e) {
    const t = this.service.getDependencyRootPaths(),
      s = this.service.getHarModuleDependenciesWithRootPath(),
      i = this.service.getHarModuleDependencyPaths(),
      o = this.service.getHspModuleDependencies(),
      r = this.service.getHspModuleDependencyPaths(),
      n = hsp_target_utils_js_1.HspTargetUtils.calDepHspTargets(this.targetService);
    return t.map(t => {
      var a, l;
      const d = inject_util_js_1.InjectUtil.getBuildCacheParentDir(t, path_1.join(hvigor_1.hvigorCore.getProject().getName(), t.substring(hvigor_1.hvigorCore.getProject().getNodeDir().length)));
      if (r.includes(t)) {
        const s = (null !== (a = o.find(([, e]) => e.getDependencyRootPath() === t)) && undefined !== a ? a : [""])[0],
          i = n.get(s);
        return path_1.resolve(d, this._buildRoot, this.targetData.getProduct().name, build_directory_const_js_1.BuildDirConst.INTERMEDIATES, build_directory_const_js_1.BuildDirConst.INTERMEDIATES_MERGE_PROFILE, null !== (l = null == i ? undefined : i.getTargetName()) && undefined !== l ? l : common_const_js_1.DefaultTargetConst.DEFAULT_TARGET, e);
      }
      return i.includes(t) ? this.getHarDenPath(s, t, d, e) : path_1.resolve(t, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, e);
    });
  }
  transformJson2Opts() {
    return this._ohLibs.map(e => fs.pathExistsSync(e) ? json_util_js_1.JsonUtil.getJson5Obj(e) : (_log.warn(`${e} does not exist. This library will not be merged. This may occur when the project is manually modified and the local library is not declared in ${this._projectModel.getProfilePath()}. Please confirm the correctness of this module. You may try to solve the problem by declare it in ${this.moduleModel.getName()} build-profile.json5.`), null));
  }
  mergeHarRequestPermission(e, t) {
    return this.mergeArrayOptions(e, t.module.requestPermissions);
  }
  mergeArrayOptions(e, t, s) {
    const i = [],
      o = [];
    return null == e || e.forEach(e => {
      i.push(e), o.push(e.name);
    }), null == t || t.forEach(e => {
      const t = e.name;
      undefined !== t && "" !== t ? o.includes(e.name) || i.push(e) : _log.warn(`The metadata configuration in module.json5 whose name is empty or undefined in the har:${s}.\n        This configuration will not be merged.`);
    }), i;
  }
  checkHarApiStatus(e) {
    var t;
    let s = e.app.minAPIVersion;
    const i = null !== (t = e.module.packageName) && undefined !== t ? t : e.module.name;
    undefined === s && _log.printErrorExit("EMPTY_MIN_API_VERSION", [`${i}`]), s.toString().length >= 4 && (s = this.harMinApiVersionToAPi(s.toString())), this.targetData.getCompatibleApiVersion() < (0, wdk_1.parseInt)(s) && _log.printErrorExit("HAR_UNSUPPORTED_API_VERSION", [this.compatibleApiVersion, s, `${i}`], [[this.projectModel.getProfilePath()], [i, this.compatibleApiVersion], [i, this.compatibleApiVersion]]);
  }
  harMinApiVersionToAPi(e) {
    const t = e.length,
      s = e.substring(t - 3, t);
    return (0, wdk_1.parseInt)(s);
  }
  customizeModuleOpt(e, t) {
    var s, i, o, r, n, a;
    const l = (0, wdk_1.cloneDeep)(null === (s = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === s ? undefined : s.appOpt),
      d = e.module,
      u = null !== (r = null !== (o = null !== (i = null == l ? undefined : l.bundleType) && undefined !== i ? i : t.bundleType) && undefined !== o ? o : e.app.bundleType) && undefined !== r ? r : "app",
      _ = this.getMergedAppOptObj(e, u, l, t);
    u === common_const_js_1.BundleType.ATOMIC_SERVICE && (d.atomicService = null !== (a = null === (n = this.targetData.getTargetOpt().config) || undefined === n ? undefined : n.atomicService) && undefined !== a ? a : d.atomicService, d.installationFree = true), u !== common_const_js_1.BundleType.APP && u !== common_const_js_1.BundleType.SHARED || (d.installationFree = false, d.atomicService && delete d.atomicService), _.buildMode = this.targetService.isDebug() ? build_mode_const_js_1.BuildModeConst.DEBUG : build_mode_const_js_1.BuildModeConst.RELEASE, undefined === _.debug && (_.debug = this.targetService.isDebug());
  }
  getMergedAppOptObj(e, t, s, i) {
    var o, r, n, a, l, d, u, _, p;
    const c = e.app;
    c.bundleType = t, this._isHarModule || (c.label = null !== (r = null !== (o = null == s ? undefined : s.label) && undefined !== o ? o : i.label) && undefined !== r ? r : c.label, c.icon = null !== (a = null !== (n = null == s ? undefined : s.icon) && undefined !== n ? n : i.icon) && undefined !== a ? a : c.icon);
    const h = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.BUILD_VERSION);
    return c.versionCode = null !== (d = null !== (l = null == s ? undefined : s.versionCode) && undefined !== l ? l : i.versionCode) && undefined !== d ? d : c.versionCode, c.versionName = null !== (_ = null !== (u = null == s ? undefined : s.versionName) && undefined !== u ? u : i.versionName) && undefined !== _ ? _ : c.versionName, c.buildVersion = null !== (p = null != h ? h : i.buildVersion) && undefined !== p ? p : c.buildVersion, c;
  }
  mergeBuildProfileAbilities(e) {
    var t, s, i;
    if (!this.moduleModel.isHapModule() || !this._moduleBuildAbilities || !e.module.abilities) return this.moduleModel.isHspModule() && this._moduleBuildAbilities && _log.warn("hsp module does not support custom icon/label/lauchType in buildProfile"), e;
    const o = e.module.abilities.map(e => e.name);
    for (const r of this._moduleBuildAbilities) for (const n of e.module.abilities) r.name && o.includes(r.name) ? n.name === r.name && (n.icon = null !== (t = r.icon) && undefined !== t ? t : n.icon, n.label = null !== (s = r.label) && undefined !== s ? s : n.label, n.launchType = null !== (i = r.launchType) && undefined !== i ? i : n.launchType) : _log.warn(`The ability name :${r.name} in build-profile can't find correspond ability name\n            in module.json.`);
    return e;
  }
  initResourceStr() {
    [...this.service.getHspDependencies(), ...this.service.getHarDependencies()].forEach(e => {
      let t;
      t = e.isLocal() ? this.pathInfo.getDependencyResourceStr(e.getDependencyRootPath()) : e.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR ? path_1.resolve(e.getDependencyRootPath(), "src", "main", "resources") : path_1.resolve(e.getDependencyRootPath(), "src", "main", "resource"), fs_extra_1.default.existsSync(t) && fs_extra_1.default.readdirSync(t).forEach(e => {
        if (!("rawfile" !== e && "resfile" !== e)) return;
        const s = path_1.resolve(t, e, "element", "string.json"),
          i = path_1.resolve(this.pathInfo.getResourceStr(), e, "element", "string.json");
        let o;
        const r = json_util_js_1.JsonUtil.getJson5Obj(s);
        this.checkJsonFormat(r, s), fs_extra_1.default.existsSync(i) ? (o = json_util_js_1.JsonUtil.getJson5Obj(i), this.checkJsonFormat(o, i), o = (0, json_util_js_1.mergeStringJson)(o, r)) : o = r, o && fs_extra_1.default.outputFileSync(i, JSON.stringify(o));
      });
    });
  }
  checkJsonFormat(e, t) {
    e && !Object.prototype.hasOwnProperty.call(e, "string") && _log.printErrorExit("WRONG_STRING_JSON_FORMAT", [t]);
  }
}
function checkHarDeviceTypes(e, t, s) {
  const i = s.getTargetData().getTargetDeviceType(),
    o = t.module.deviceTypes,
    r = "default",
    n = "phone";
  (o.includes(r) || o.includes(n)) && (o.push(n), o.push(r));
  const a = i.filter(e => !o.includes(e));
  a.length > 0 && _log.printErrorExit("CURRENT_MODULE_NOT_SUPPORT_THE_DEVICE_TYPE", [t.module.type, t.module.name, a.join(", ")]);
}
__decorate([(0, hvigor_1.Input)()], MergeProfile.prototype, "includeAppScopeRes", null);
