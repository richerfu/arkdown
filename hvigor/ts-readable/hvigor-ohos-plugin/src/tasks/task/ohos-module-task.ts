/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/task/ohos-module-task.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as fse from "fs-extra";
import * as wdk_1 from "@baize/wdk";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as file_util_js_1 from "../../utils/file-util";
import * as json_util_js_1 from "../../utils/json-util";
import * as oh_package_loader_js_1 from "../../utils/loader/file/oh-package-loader";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as oh_exports_util_js_1 from "../../utils/oh-exports-util";
import * as dependency_interface_js_1 from "../../project/dependency/core/dependency-interface";
export class OhosModuleTask extends hvigor_1.IncrementalExecTask {
  constructor(e, t) {
    super(e.getTargetData().getModuleModel().getModule(), e.buildTaskName(t)), this.ohosLogger = ohos_logger_js_1.OhosLogger.getLogger(OhosModuleTask.name), this.paramWhiteList = ["version", "devDependencies", "dynamicDependencies", "dependencies"], this.localProductDependenciesPath = [], this.registryAction = () => async () => {
      await this.beforeTask(), this.taskShouldDo() && (await this.doTaskAction());
    }, this.module = e.getTargetData().getModuleModel().getModule(), this.moduleName = this.module.getName(), this.targetService = e, this.targetData = e.getTargetData(), this.service = e.getModuleService(), this.projectModel = this.service.getProjectModel(), this.moduleModel = this.service.getModuleModel(), this.sdkInfo = this.targetService.getSdkInfo(), this.isFaMode = this.service.isFaMode(), this.targetName = this.targetService.getTargetData().getTargetName(), this.pathInfo = e.getTargetData().getPathInfo(), this.compileApiVersion = this.targetData.getCompileApiVersion(), this.compatibleApiVersion = this.targetData.getCompatibleApiVersion(), this.originCompatibleSdkVersion = this.targetData.getOriginCompatibleSdkVersion(), this.targetSdkVersion = this.targetData.getTargetSdkVersion(), this.compatibleSdkVersionStage = this.targetData.getCompatibleSdkVersionStage(), this.moduleModelNodePaths = new Set(this.projectModel.getAllModules().map(e => e.getModule().getNodeDir())), this.appStartupPath = e.getAppStartupPath(this.isFaMode), this.appStartupFileName = e.getAppStartupFileName(this.isFaMode), this.intermediateTempStartupFilePath = this.pathInfo.getIntermediatesStartupPath(this.appStartupFileName), this.isOhpmProject = this.projectModel.isOhpmProject(), this.packageManagerPath = this.isOhpmProject ? this.moduleModel.getOhPackageJson5Path() : this.moduleModel.getPackageJsonPath(), this.initHarModuleDepends(), hvigor_1.hvigor.nodesEvaluatedInternal(() => {
      this.initTaskDependsForOtherModule();
    }), this.getWorkerPool().warmUp(common_const_js_1.CommonConst.WARM_UP_SCRIPT), this.shouldCloseWorkerPoolAfterBuild() && this.getWorkerPool().setResident(false), this.packageJsonObj = hvigor_1.Json5Reader.getJson5Obj(this.packageManagerPath);
    const s = this.projectModel.getParameterFileObj();
    if (this.isOhpmProject) {
      const e = {
        parameterFilePath: this.projectModel.getParameterFileAbsolutePath(),
        ohPackageFilePath: this.packageManagerPath
      };
      file_util_js_1.FileUtil.resolveJsonObjWithoutParam(this.packageJsonObj, s, e, this.paramWhiteList);
    }
  }
  get isPreloadSystemSoEnabled() {
    var e;
    return !!(null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.preloadSystemSo);
  }
  get isPreloadSystemSoJsonEffective() {
    var e, t;
    return !!(null === (t = null === (e = this.getPreloadSystemSoJson()) || undefined === e ? undefined : e.systemPreloadHintStartupTasks) || undefined === t ? undefined : t.length);
  }
  getPreloadSystemSoJson() {
    if (!this.preloadSystemSoJson) {
      const e = this.pathInfo.getPreloadSoFilePath();
      this.preloadSystemSoJson = this.isPreloadSystemSoEnabled && fse.existsSync(e) ? json_util_js_1.JsonUtil.getJson5Obj(e) : undefined;
    }
    return this.preloadSystemSoJson;
  }
  addPreloadSystemSoJson(e, t) {
    var s;
    const o = `${null !== (s = this.appStartupFileName) && undefined !== s ? s : build_directory_const_js_1.BuildDirConst.DEFAULT_STARTUP_CONFIG_FILE_NAME}.json`,
      i = path_1.resolve(this.pathInfo.getIntermediatesResProfilePath(), o);
    let a = fse.existsSync(i) ? json_util_js_1.JsonUtil.getJson5Obj(i) : undefined;
    a ? a.systemPreloadHintStartupTasks = [...(a.systemPreloadHintStartupTasks || []), ...(e.systemPreloadHintStartupTasks || [])] : a = e, a.systemPreloadHintStartupTasks = (0, wdk_1.uniqBy)(a.systemPreloadHintStartupTasks || [], "name");
    const r = path_1.relative(this.pathInfo.getIntermediatesRes(), i),
      n = path_1.resolve(t, r);
    fse.ensureDirSync(path_1.dirname(n)), fse.writeFileSync(n, JSON.stringify(a));
  }
  shouldCloseWorkerPoolAfterBuild() {
    return (0, hvigor_1.isLinux)() || this.isFaMode || this.sdkInfo.isOhos || this.sdkInfo.getSdkVersion() < sdk_const_js_1.ApiVersion.API_VERSION_11;
  }
  initTaskDependsForOtherModule() {}
  initHarModuleDepends() {}
  beforeTask() {
    if (this.isOhpmProject) {
      const e = {
          parameterFilePath: this.projectModel.getParameterFileAbsolutePath(),
          ohPackageFilePath: this.packageManagerPath
        },
        t = file_util_js_1.FileUtil.resolveJsonObjWithoutParam(this.packageJsonObj, this.projectModel.getParameterFileObj(), e, this.paramWhiteList, e => oh_package_loader_js_1.ohPackageLoader.getOhPackageJsonObj(e));
      this.localProductDependenciesPath.push(...t);
    }
  }
  taskShouldDo() {
    return true;
  }
  getTaskTempDir(e, t, s = true) {
    const o = path_1.resolve(e.getPathInfo().getModuleBuildCachePath(), t || this.name);
    return s && !fse.existsSync(o) && fse.mkdirsSync(o), o;
  }
  getImporteesCachePath(e, t) {
    const s = this.targetService.getTargetData(),
      o = this.moduleModel.getCompileMode(s.getProduct());
    return path_1.resolve(this.getTaskTempDir(s, `${this.targetName}@${t}`, false), o, this.targetService.getBuildMode().toLowerCase(), e);
  }
  processImportees(e, t, s) {
    if (!fse.existsSync(e)) return;
    const o = json_util_js_1.JsonUtil.getJson5Obj(e);
    Array.isArray(o) && o.forEach(e => {
      const o = t.find(t => e === t.getDependencyName() || e.startsWith(`${t.getDependencyName()}/`));
      o && s(e, o.getDependencyType(), o);
    });
  }
  getGlobalDependencyKeys() {
    const e = this.moduleModel.getModule().getNodeDir();
    return this.service.getAllDirectPkgNodes().filter(t => !!t.parent && t.parent.npm.getDependencyRootPath() !== e).map(e => e.npm.getDependencyName());
  }
  collectDirectAffectsCompilationDependencyKeys(e) {
    var t, s, o;
    const i = this.getGlobalDependencyKeys(),
      a = new Set(null === (o = null === (s = null === (t = this.targetService.getBuildOption()) || undefined === t ? undefined : t.arkOptions) || undefined === s ? undefined : s.runtimeOnly) || undefined === o ? undefined : o.packages);
    this.processImportees(e, this.service.getAllDirectDependencies(), (e, t, s) => {
      a.add(s.getDependencyName());
    });
    const r = Array.from(a),
      n = r.filter(e => i.includes(e));
    return {
      compileDependencyKeys: r,
      globalCompileDependencyKeys: n
    };
  }
  doGlobalLocalDependencyCheck(e) {
    if (this.targetName === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET) return;
    const {
        compileDependencyKeys: t,
        globalCompileDependencyKeys: s
      } = this.collectDirectAffectsCompilationDependencyKeys(e),
      o = new Set();
    this.service.getAllDirectDependencies().forEach(e => {
      const i = e.getDependencyName();
      t.includes(i) && s.includes(i) && e.isLocal() && o.add(i);
    }), o.size && this.ohosLogger.warn(`You have used local dependencies [${[...o].join(",")}]. These dependencies are not configured in the oh-package.json5 file of the current module, therefore, the build system does not collect their libs, resources, and related configurations (such as dynamicImport, routerMap, and abilities) during the build process. As a result, a runtime exception may occur during subsequent integration. You are advised to configure these dependencies in the oh-package.json5 file of the current module.`);
  }
  doDevDependencyCheck(e) {
    if (this.targetName === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET) return;
    const {
        compileDependencyKeys: t
      } = this.collectDirectAffectsCompilationDependencyKeys(e),
      s = new Set();
    this.service.getAllDirectDependencies().forEach(e => {
      const o = e.getDependencyName();
      t.includes(o) && e.getDependencyEnum() === dependency_interface_js_1.DependencyEnum.DEV_DEPENDENCIES && s.add(o);
    }), s.size && this.ohosLogger.warn(`The dependencies [${[...s].join(",")}] you are using are listed under devDependencies. As such, the build system won't collect their libs, resources, and related configurations (for dynamicImport, routerMap, abilities, and more) during the build process. This could cause runtime issues later. To avoid these issues, move these dependencies to 'dependencies'.`);
  }
  replacePagesInPreview(e, t) {
    const s = path_1.resolve(this.pathInfo.getIntermediatesRes(), common_const_js_1.CommonConst.MODULE_JSON),
      o = json_util_js_1.JsonUtil.getJson5Obj(s),
      i = path_1.resolve(this.projectModel.getProjectDir(), this.moduleModel.getName(), `src/main/${build_directory_const_js_1.BuildDirConst.RESOURCES_BASE_PROFILE_DIR}/${build_directory_const_js_1.BuildArtifactConst.MAIN_PAGES_JSON}`);
    if (this.moduleModel.getModuleType() === module_type_enum_js_1.ModuleType.Shared && !fse.existsSync(i)) {
      const e = this.pathInfo.getPreviewIntermediatesMainPagesJsonPath();
      fse.writeJSONSync(e, {
        src: ["pages/Index"]
      });
    }
    const a = `${o.module.pages.replace(/\$profile:/, "")}.json`,
      r = path_1.resolve(this.pathInfo.getIntermediatesResProfilePath(), a),
      n = fse.readJSONSync(r);
    n.src = t, e.debug(`Resolved file ${r}.`), e.debug(`Replace page to ${t}`), fse.outputJSONSync(r, n);
  }
  replacePages(e, t) {
    const s = {
        src: t
      },
      o = this.getPageJsonFileName(e),
      i = path_1.resolve(this.pathInfo.getIntermediatesResProfilePath(), o);
    e.debug(`Resolved file ${i}.`), e.debug(`Replace page to ${t}`), fse.outputJSONSync(i, s);
  }
  getPageJsonFileName(e) {
    const t = path_1.resolve(this.pathInfo.getIntermediatesRes(), common_const_js_1.CommonConst.MODULE_JSON),
      s = json_util_js_1.JsonUtil.getJson5Obj(t).module.pages;
    return s || e.printErrorExit("UNABLE_TO_REPLACE_PAGES_CANNOT_FIND_PAGES", [this.moduleModel.getName()]), `${s.replace(/\$profile:/, "")}.json`;
  }
  getDependencies(e, t) {
    const s = this.projectModel.isOhpmProject() ? e.getOhPackageJson5Path() : e.getPackageJsonPath();
    let o = {};
    if (fse.existsSync(s)) {
      const e = hvigor_1.Json5Reader.getJson5Obj(s);
      o = {
        dependency: o,
        ...e.dependencies
      }, t && Object.assign(o, e.devDependencies);
    }
    const i = {};
    return Object.keys(o).sort().forEach(e => i[e] = o[e]), i;
  }
  getDynamicDependencies(e) {
    const t = this.projectModel.isOhpmProject() ? e.getOhPackageJson5Path() : e.getPackageJsonPath();
    let s = {};
    if (fse.existsSync(t)) try {
      const e = hvigor_1.Json5Reader.getJson5Obj(t);
      s = {
        dependency: s,
        ...e.dynamicDependencies
      };
    } catch (e) {
      this.ohosLogger.warn("Failed to read json5:", e.message);
    }
    const o = {};
    return Object.keys(s).sort().forEach(e => o[e] = s[e]), o;
  }
  declareDepends(e, t) {
    let s;
    return s = e instanceof hvigor_1.CoreTask ? e.getName() : e, s.split("@").length > 1 || (s = `${this.targetName}@${s}`), super.dependsOn(s, t);
  }
  declareDependsList(...e) {
    e.forEach(e => {
      this.declareDepends(e);
    });
  }
  dependsOnHook(e, t) {
    let s;
    return s = e instanceof hvigor_1.CoreTask ? e.getName() : e, super.dependsOn(s, t);
  }
  hasUseTsHarField() {
    var e;
    if (this.targetService.isByteCodeHar()) return true;
    if (!this.isFaMode) {
      return undefined !== (null === (e = this.moduleModel.getJsonProfileOptByTargetName(this.targetName, false).module.metadata) || undefined === e ? undefined : e.find(e => e.name === common_const_js_1.CommonConst.USE_TS_HAR && "true" === e.value));
    }
    return false;
  }
  getOhExportsFilePaths() {
    const e = this.packageJsonObj[common_const_js_1.CommonConst.OH_EXPORTS];
    if (e) return (0, oh_exports_util_js_1.collectOhExportsFilePaths)(this.moduleModel.getProjectDir(), e);
  }
}
