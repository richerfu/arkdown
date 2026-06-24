/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/generate-pkg-context-info.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as project_ohos_config_manager_js_1 from "../common/global/project-ohos-config-manager";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as collect_duplicate_har_dependencies_js_1 from "../har-dedepulicate/collect_duplicate_har_dependencies";
import * as dependency_interface_js_1 from "../project/dependency/core/dependency-interface";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
import * as cmake_util_js_1 from "../utils/cmake/cmake-util";
import * as copy_resources_util_js_1 from "../utils/copy-resources-util";
import * as file_util_js_1 from "../utils/file-util";
import * as har_target_util_js_1 from "../utils/har-target-util";
import * as hsp_target_utils_js_1 from "../utils/hsp-target-utils";
import * as inject_util_js_1 from "../utils/inject-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as ohmurl_utils_js_1 from "../utils/ohmurl-utils";
import * as ohos_test_util_js_1 from "../utils/ohos-test-util";
import * as process_libs_common_properties_js_1 from "./common-properties/process-libs-common-properties";
import * as task_names_js_1 from "./common/task-names";
var __decorate = this && this.__decorate || function (e, t, o, s) {
  var n,
    i = arguments.length,
    r = i < 3 ? t : null === s ? s = Object.getOwnPropertyDescriptor(t, o) : s;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) r = Reflect.decorate(e, t, o, s);else for (var a = e.length - 1; a >= 0; a--) (n = e[a]) && (r = (i < 3 ? n(r) : i > 3 ? n(t, o, r) : n(t, o)) || r);
  return i > 3 && r && Object.defineProperty(t, o, r), r;
};
export const soLibsPattern = /^.*\.so(\.[0-9])*$/;
export class GeneratePkgContextInfo extends process_libs_common_properties_js_1.ProcessLibsCommonProperties {
  get useNormalizedOHMUrl() {
    var e, t;
    return !!(null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.strictMode) || undefined === t ? undefined : t.useNormalizedOHMUrl);
  }
  declareInputFiles() {
    const e = super.declareInputFiles();
    return this.targetService.shouldDoDeduplicateTask() && this.moduleModel.isHapModule() && this.processHarDeduplicateInputFiles(e), e;
  }
  declareOutputFiles() {
    const e = new hvigor_1.FileSet().addEntry(this.pkgInfoPath, {
      isDirectory: false
    });
    return this.duplicatePkgContextInfoObj && Object.keys(this.duplicatePkgContextInfoObj).length > 0 && e.addEntry(this.pathInfo.getPkgContextInfoDuplicateInfoPath()), e;
  }
  declareInputs() {
    const e = super.declareInputs();
    return e.set("pkgContextInfoMap", JSON.stringify(Object.fromEntries(this.pkgContextInfoMap))), e;
  }
  constructor(e) {
    var t, o, s, n, i, r;
    super(e, task_names_js_1.TaskNames.Task.GENERATE_PKG_CONTEXT_INFO), this.pkgContextInfoMap = new Map(), this._log = ohos_logger_js_1.OhosLogger.getLogger(GeneratePkgContextInfo.name);
    const a = this.projectModel.getAppRes().getAppResOpt(),
      l = (0, wdk_1.cloneDeep)(null === (t = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === t ? undefined : t.appOpt),
      d = this.targetData.getProduct();
    if (this.currentBundleName = null !== (s = null !== (o = null == l ? undefined : l.bundleName) && undefined !== o ? o : this.targetData.getProduct().bundleName) && undefined !== s ? s : this.projectModel.getDefaultBundleName(), this.currentBundleType = null !== (r = null !== (i = null !== (n = null == l ? undefined : l.bundleType) && undefined !== n ? n : d.bundleType) && undefined !== i ? i : a.app.bundleType) && undefined !== r ? r : common_const_js_1.BundleType.APP, this.pkgInfoPath = this.pathInfo.getIntermediatesPkgContextInfoPath(), fs_extra_1.existsSync(path_1.dirname(this.pkgInfoPath)) || fs_extra_1.mkdirSync(path_1.dirname(this.pkgInfoPath), {
      recursive: true
    }), this.targetService.shouldDoDeduplicateTask() && this.moduleModel.isHapModule()) {
      const e = (0, collect_duplicate_har_dependencies_js_1.getMergedHapAllHspDependencies)();
      this.allHspDepTargets = hsp_target_utils_js_1.HspTargetUtils.calHspTargets(this.targetService, e);
    }
  }
  async beforeAlwaysAction() {
    await super.beforeAlwaysAction(), this.initPkgContextInfo();
  }
  beforeTask() {
    fs_extra_1.existsSync(this.pkgInfoPath) && fs_extra_1.removeSync(this.pkgInfoPath);
  }
  taskShouldDo() {
    return this.useNormalizedOHMUrl;
  }
  doTaskAction() {
    const e = JSON.stringify(Object.fromEntries(this.pkgContextInfoMap));
    fs_extra_1.writeFileSync(this.pkgInfoPath, e), this.targetService.shouldDoDeduplicateTask() && this.moduleModel.isHspModule() && this.writeDuplicatePkgContextInfoToIntermediateFile();
  }
  initPkgContextInfo() {
    const e = new dependency_manager_js_1.DependencyManager(this.projectModel.isFaMode(), this.moduleModel, this.projectModel);
    let {
      npmDependencies: t
    } = e.collectAllDependenciesForPkgInfo();
    if (inject_util_js_1.InjectUtil.isLocalTest() || inject_util_js_1.InjectUtil.isPreviewProcess()) {
      const o = e.collectAllDependencies().npmDependencies.filter(e => e.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP);
      t = t.concat(o);
    }
    for (const e of t) this.pkgContextInfoMap.set(e.getPackageName(), this.getDependenciesPkgContextInfo(e));
    this.pkgContextInfoMap.set(this.packageJsonObj.name, this.getCurrentPkgContextInfo()), this.initLibsSoPkgContextInfo(), this.targetService.shouldDoDeduplicateTask() && (this.moduleModel.isHspModule() ? this.processHspHarDeduplicate() : this.moduleModel.isHapModule() && this.addDuplicateHarPkgContextInfo());
  }
  getCurrentPkgContextInfo() {
    const e = {
      packageName: this.packageJsonObj.name,
      bundleName: this.getCurrentModuleBundleName(),
      moduleName: "",
      version: this.moduleModel.isHarModule() ? this.packageJsonObj.version : "",
      entryPath: this.getCurrentEntryPath(this.moduleModel.getProjectDir(), this.packageJsonObj),
      isSO: false,
      dependencyAlias: ""
    };
    return (this.moduleModel.isHarModule() || this.moduleModel.isHspModule()) && this.writeOhExportsOhmUrlToPkgContextInfo(this.moduleModel.getProjectDir(), this.getOhExportsFilePaths(), e), e;
  }
  getCurrentEntryPath(e, t) {
    if (this.moduleModel.isHapModule() || this.targetName === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET) {
      const t = this.moduleModel.getSourceRootByTargetName(this.targetName);
      if (this.targetName === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET && (0, ohos_test_util_js_1.isTemplatesAfterSourceCodeMigration)(t)) {
        const e = (0, ohos_test_util_js_1.getOhosTestIntermediatesSrcOhosTestRelativePath)();
        return `${(0, copy_resources_util_js_1.toUnixPath)(e)}/`;
      }
      return t ? `${(0, copy_resources_util_js_1.toUnixPath)(path_1.relative(e, t))}/` : "";
    }
    if (this.moduleModel.isHarModule()) {
      const o = (0, dependency_manager_js_1.getEntryMainFilePath)(e, t);
      return o ? (0, copy_resources_util_js_1.toUnixPath)(path_1.relative(e, path_1.resolve(e, o))) : "";
    }
    return "";
  }
  getDependenciesPkgContextInfo(e) {
    var t, o, s;
    const n = e.getDependencyType(),
      i = {
        packageName: e.getPackageName(),
        bundleName: this.getBundleName(e, n),
        moduleName: n === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP && null !== (o = null === (t = e.getModuleJsonObj()) || undefined === t ? undefined : t.module.name) && undefined !== o ? o : "",
        version: this.getVersion(e, n),
        entryPath: (null !== (s = e.getDependencyMainFileRelativePath()) && undefined !== s ? s : "").replace(/\.d\.e?ts$/, ""),
        isSO: n === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_SO,
        dependencyAlias: this.getDependencyAlias(e)
      };
    return this.writeOhExportsOhmUrlToPkgContextInfo(e.getDependencyRootPath(), e.getOhExportsFilePaths(), i), i;
  }
  writeOhExportsOhmUrlToPkgContextInfo(e, t, o) {
    const s = this.generateOhExportsOhmUrlObj(e, t, o);
    s && (o[common_const_js_1.CommonConst.OH_EXPORTS] = s);
  }
  generateOhExportsOhmUrlObj(e, t, o) {
    if (!t) return;
    const s = {};
    return t.forEach(t => {
      const n = path_1.relative(e, t);
      s[(0, copy_resources_util_js_1.toUnixPath)(n)] = (0, ohmurl_utils_js_1.generateOhmUrlForSourceFile)(o, n);
    }), s;
  }
  isBundleTypeSharedOrAppPlugin(e) {
    return e === common_const_js_1.BundleType.SHARED || e === common_const_js_1.BundleType.APP_PLUGIN;
  }
  getBundleName(e, t) {
    var o, s, n, i;
    const r = e.getModuleJsonObj();
    return this.isBundleTypeSharedOrAppPlugin(this.currentBundleType) ? t !== dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP || dependency_manager_js_1.DependencyManager.isLocalDependency(e, this.moduleModelNodePaths) ? this.currentBundleName : this.isHspInCommonApp(r, t) && r ? null !== (i = null === (n = r.app) || undefined === n ? undefined : n.bundleName) && undefined !== i ? i : "" : (this._log.printErrorExit("REMOTE_HSP_NOT_ALLOWED", [e.getPackageName()], [[e.getPackageName()]]), "") : r && this.isHspInCommonApp(r, t) && (null === (o = r.app) || undefined === o ? undefined : o.bundleName) !== this.currentBundleName ? null === (s = r.app) || undefined === s ? undefined : s.bundleName : "";
  }
  getVersion(e, t) {
    if (t === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR || t === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_OTHER) {
      const t = {
        parameterFilePath: this.service.getProjectModel().getParameterFileAbsolutePath(),
        ohPackageFilePath: this.isOhpmProject ? this.moduleModel.getOhPackageJson5Path() : this.moduleModel.getPackageJsonPath()
      };
      return file_util_js_1.FileUtil.extracted(e.getDependencyVersion(), this.moduleModel.getParentProject().getParameterFileObj(), t);
    }
    return "";
  }
  getDependencyAlias(e) {
    return e.getDependencyName() === e.getPackageName() ? "" : e.getDependencyName();
  }
  isHspInCommonApp(e, t) {
    var o;
    return t === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP && this.isBundleTypeSharedOrAppPlugin(null === (o = e.app) || undefined === o ? undefined : o.bundleType);
  }
  initLibsSoPkgContextInfo() {
    this.getNativeLibrary(this.targetService);
    const e = har_target_util_js_1.HarTargetUtil.calDepHarTargets(this.targetService);
    for (const [t, o] of e) {
      const e = this.projectModel.getTarget(t, o);
      e && (this.getNativeLibrary(e), this.getNativeLibraryByLibs(path_1.resolve(e.getModuleService().getModuleModel().getProjectDir(), build_directory_const_js_1.BuildDirConst.LIBS)));
    }
    this.getNativeLibraryByLibs(path_1.resolve(this.moduleModel.getProjectDir(), build_directory_const_js_1.BuildDirConst.LIBS)), this.service.getHarDependencies().filter(e => !e.isLocal()).forEach(e => {
      this.getNativeLibraryByLibs(e.getDefaultLibsDir());
    });
  }
  getNativeLibraryByLibs(e) {
    this.getLibsInfo(e).map(t => ({
      name: path_1.basename(t),
      path: path_1.resolve(e, t)
    })).filter(e => fs_extra_1.lstatSync(e.path).isFile()).forEach(e => {
      this.setPkgContextInfoMapWithSo(e.name);
    });
  }
  getNativeLibrary(e) {
    const t = e.getTargetData().getPathInfo().getNinjaWorkDir(),
      o = e.getBuildOption().externalNativeOptions;
    if (!e.getShouldBuildCmake() || !this.useNormalizedOHMUrl) return;
    const s = cmake_util_js_1.CmakeUtil.checkAbiFilters(null == o ? undefined : o.abiFilters, e.getTargetData().isHarmonyOS(), e.getModuleService().getModuleModel(), e.getTargetData().getTargetName());
    for (const e of s) {
      const o = path_1.resolve(t, e);
      cmake_util_js_1.CmakeUtil.parseLibraries(o, this.targetName, e).forEach(e => {
        const t = e.getNameOnDisk();
        t && this.setPkgContextInfoMapWithSo(t);
      });
    }
  }
  setPkgContextInfoMapWithSo(e) {
    const t = this.getCurrentModuleBundleName();
    this.pkgContextInfoMap.has(e) || this.pkgContextInfoMap.set(e, getSoDefaultContextInfo(e, t));
  }
  getCurrentModuleBundleName() {
    return this.moduleModel.isHspModule() && this.isBundleTypeSharedOrAppPlugin(this.currentBundleType) ? this.currentBundleName : "";
  }
  addDuplicateHarPkgContextInfo() {
    this.allHspDepTargets && this.allHspDepTargets.forEach(e => {
      const t = e.getPathInfo().getPkgContextInfoDuplicateInfoPath();
      if (!fs_extra_1.existsSync(t)) return;
      const o = fs_extra_1.readJSONSync(t);
      o && Object.keys(o).forEach(e => {
        this.pkgContextInfoMap.has(e) || (this.pkgContextInfoMap.set(e, o[e]), this._log.debug(`Deduplicate add pkgContextInfo ${e} ${o[e]}`));
      });
    });
  }
  processHspHarDeduplicate() {
    const e = (0, collect_duplicate_har_dependencies_js_1.getHspNeedDeduplicateHarInfoMap)(this.moduleName);
    if (!e) return;
    const t = (0, collect_duplicate_har_dependencies_js_1.getMergedHap)(),
      o = new Set(Object.values(e).map(e => e.packageName)),
      s = {};
    o.forEach(e => {
      const o = this.pkgContextInfoMap.get(e);
      o && (s[e] = {
        ...o
      }, o.moduleName = t, this._log.debug(`Deduplicate pkgContextInfo ${o}`));
    }), this.duplicatePkgContextInfoObj = s;
  }
  writeDuplicatePkgContextInfoToIntermediateFile() {
    this.duplicatePkgContextInfoObj && Object.keys(this.duplicatePkgContextInfoObj).length > 0 && fs_extra_1.outputJSONSync(this.pathInfo.getPkgContextInfoDuplicateInfoPath(), this.duplicatePkgContextInfoObj);
  }
  processHarDeduplicateInputFiles(e) {
    this.allHspDepTargets && this.allHspDepTargets.forEach(t => {
      const o = t.getPathInfo().getPkgContextInfoDuplicateInfoPath();
      fs_extra_1.existsSync(o) && e.addEntry(o);
    });
  }
  initTaskDepends() {}
}
export function getSoDefaultContextInfo(e, t) {
  return {
    packageName: e,
    bundleName: null != t ? t : "",
    moduleName: "",
    version: "",
    entryPath: "",
    isSO: true,
    dependencyAlias: ""
  };
}
__decorate([(0, hvigor_1.Input)()], GeneratePkgContextInfo.prototype, "useNormalizedOHMUrl", null);
