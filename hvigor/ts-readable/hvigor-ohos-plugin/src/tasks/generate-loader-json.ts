/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/generate-loader-json.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import * as fse from "fs-extra";
import * as project_ohos_config_manager_js_1 from "../common/global/project-ohos-config-manager";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as inject_const_js_1 from "../const/inject-const";
import * as sdk_const_js_1 from "../const/sdk-const";
import * as aot_compile_mode_enum_js_1 from "../enum/aot-compile-mode-enum";
import * as code_type_enum_js_1 from "../enum/code-type-enum";
import * as module_type_enum_js_1 from "../enum/module-type-enum";
import * as collect_duplicate_har_dependencies_js_1 from "../har-dedepulicate/collect_duplicate_har_dependencies";
import * as build_mode_manager_js_1 from "../project/build-option/build-mode-manager";
import * as dependency_interface_js_1 from "../project/dependency/core/dependency-interface";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
import * as array_util_js_1 from "../utils/array-util";
import * as build_profile_utils_js_1 from "../utils/build-profile-utils";
import * as byte_code_har_utils_js_1 from "../utils/byte-code-har-utils";
import * as file_util_js_1 from "../utils/file-util";
import * as har_target_util_js_1 from "../utils/har-target-util";
import * as hsp_target_utils_js_1 from "../utils/hsp-target-utils";
import * as inject_util_js_1 from "../utils/inject-util";
import * as json_util_js_1 from "../utils/json-util";
import * as oh_package_loader_js_1 from "../utils/loader/file/oh-package-loader";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as ohmurl_utils_js_1 from "../utils/ohmurl-utils";
import * as strategy_js_1 from "../utils/strategy";
import * as task_util_js_1 from "../utils/task-util";
import * as sdk_version_enum_js_1 from "../version/sdk-version-enum";
import * as sdk_version_js_1 from "../version/sdk-version";
import * as abstract_generate_loader_json_js_1 from "./abstract-generate-loader-json";
import * as task_names_js_1 from "./common/task-names";
import * as deduplicate_util_js_1 from "./har-deduplicate/deduplicate-util";
import * as pre_build_js_1 from "./pre-build";
import * as target_task_service_js_1 from "./service/target-task-service";
import * as exclude_remote_dynamic_import_js_1 from "../common/exclude-resource-code/exclude-remote-dynamic-import";
import * as build_option_path_info_js_1 from "../common/build-option-path-info";
var __decorate = this && this.__decorate || function (e, t, i, s) {
  var o,
    n = arguments.length,
    r = n < 3 ? t : null === s ? s = Object.getOwnPropertyDescriptor(t, i) : s;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) r = Reflect.decorate(e, t, i, s);else for (var a = e.length - 1; a >= 0; a--) (o = e[a]) && (r = (n < 3 ? o(r) : n > 3 ? o(t, i, r) : o(t, i)) || r);
  return n > 3 && r && Object.defineProperty(t, i, r), r;
};
const log = ohos_logger_js_1.OhosLogger.getLogger("GenerateLoaderJson");
export class GenerateLoaderJson extends abstract_generate_loader_json_js_1.AbstractGenerateLoaderJson {
  get useNormalizedOHMUrl() {
    var e, t;
    return !!(null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.strictMode) || undefined === t ? undefined : t.useNormalizedOHMUrl);
  }
  get isByteCodeHarOptimize() {
    return inject_util_js_1.InjectUtil.isByteCodeHarOptimize();
  }
  declareInputs() {
    var e, t, i;
    const s = super.declareInputs(),
      o = this.isOhpmProject ? this.moduleModel.getOhPackageJson5Path() : this.moduleModel.getPackageJsonPath(),
      n = json_util_js_1.JsonUtil.getJson5Obj(o);
    s.set("module_dependencies", JSON.stringify(n.dependencies)), n[common_const_js_1.CommonConst.OH_EXPORTS] && s.set("module_oh_exports", JSON.stringify(n[common_const_js_1.CommonConst.OH_EXPORTS]));
    const r = this.isOhpmProject ? this.projectModel.getOhPackageJson5Path() : this.projectModel.getPackageJsonPath(),
      a = json_util_js_1.JsonUtil.getJson5Obj(r);
    s.set("overrides", a.overrides), s.set("project_dependencies", JSON.stringify(a.dependencies)), s.set(common_const_js_1.CommonConst.USE_NORMALIZED_OHM_URL, this.useNormalizedOHMUrl), s.set("isOhosTest", inject_util_js_1.InjectUtil.isOhosTest()), this.service.getHarDependencies().forEach(e => {
      var t;
      if (e.isByteCodeHarDependency()) return;
      const i = e.getModuleJsonObj();
      s.set("dependencyModuleAbility", JSON.stringify(null === (t = null == i ? undefined : i.module) || undefined === t ? undefined : t.abilities));
    }), s.set("isFullCompilationEnabled", this.isFullCompilationEnabled());
    const l = this.pathInfo.getBuildConfigPath();
    if (fse.existsSync(l)) {
      const e = json_util_js_1.JsonUtil.getJson5Obj(l);
      s.set("patchConfig", JSON.stringify(e.patchConfig));
    }
    return s.set("appStartupFileName", null !== (e = this.appStartupFileName) && undefined !== e ? e : ""), this.declareDynamicImportInputs(s), s.set("compileMode", this.targetCompileMode.toString()).set("nodeModulesPath", this.nodeModulesPath).set("targetConfig", JSON.stringify(this.targetData.getTargetOpt())).set("hspNameOhmMap", JSON.stringify(this.hspNameOhmMap)).set("harNameOhmMap", JSON.stringify(this.harNameOhmMap)).set("anBuildMode", null !== (t = this.targetService.getAnBuildMode()) && undefined !== t ? t : "").set("apPath", null !== (i = this.apPath) && undefined !== i ? i : "").set("fallbackAnBuild", this.fallbackAnBuild).set("isHarWithCoverage", this.isHarWithCoverage()).set("compileApiVersion", this.compileApiVersion).set("compatibleApiVersion", this.compatibleApiVersion).set("isByteCodeHar", this.targetService.isByteCodeHar()).set("shouldTreatHarAsHap", this.shouldTreatHarAsHap()).set("isBundledDependencies", this.isBundledDependencies).set("needSubmitArkTsWidget", this.needSubmitArkTsWidget).set("isCustomizedHar", this.targetService.isCustomizedHar());
  }
  declareDynamicImportInputs(e) {
    var t, i;
    const s = Object.keys(this.getDependencies(this.moduleModel, false)),
      o = Object.keys(this.getDependencies(this.projectModel, false)),
      n = (0, build_profile_utils_js_1.getRuntimeOnlyObjMap)(this.module.getNodeDir(), null === (i = null === (t = this.targetService.getBuildOption()) || undefined === t ? undefined : t.arkOptions) || undefined === i ? undefined : i.runtimeOnly, (0, array_util_js_1.getUnionSet)(o, s));
    for (const [t, i] of n) e.set(t, i);
    this.service.getHarModuleDependencies().forEach(([t, i]) => {
      var s;
      const n = (null === (s = i.getPackageJsonObj()) || undefined === s ? undefined : s.dependencies) || {},
        r = (0, build_profile_utils_js_1.getRuntimeOnlyObjMap)(i.getDependencyRootPath(), this.getLocalDependencyRuntimeOnly(i), (0, array_util_js_1.getUnionSet)(o, Object.keys(n)), t);
      for (const [t, i] of r) e.set(t, i);
    });
  }
  declareInputFiles() {
    var e, t;
    const i = new hvigor_1.FileSet();
    this.apPath && fse.existsSync(this.apPath) && i.addEntry(this.apPath, {
      isDirectory: false
    }), this.insightIntentJsonPath && fse.existsSync(this.insightIntentJsonPath) && i.addEntry(this.insightIntentJsonPath), this.mockConfigJsonPath && fse.existsSync(this.mockConfigJsonPath) && i.addEntry(this.mockConfigJsonPath), this.addStartupInputFiles(i), fse.existsSync(this.pathInfo.getIntermediatesPkgContextInfoPath()) && i.addEntry(this.pathInfo.getIntermediatesPkgContextInfoPath());
    const s = this.getModuleEntryFile();
    this.isHarWithCoverage() && fse.existsSync(s) && i.addEntry(s);
    const o = this.pathInfo.getIntermediatesRouterMap(build_directory_const_js_1.BuildArtifactConst.LOADER_ROUTER_MAP_FILE_NAME);
    if (fse.existsSync(o)) {
      (null === (e = json_util_js_1.JsonUtil.getJson5Obj(o).routerMap) || undefined === e ? undefined : e.length) && i.addEntry(o);
    }
    const n = this.pathInfo.getIntermediatesRouterMap(null !== (t = this.targetService.getRouterMapJsonFileName(this.moduleModel)) && undefined !== t ? t : build_directory_const_js_1.BuildArtifactConst.TEMP_ROUTER_MAP_FILE_NAME);
    return fse.existsSync(n) && i.addEntry(n), this.allowToCollectEtsFolder() && fse.existsSync(this.etsSourcePath) && i.addEntry(this.etsSourcePath, {
      isDirectory: true
    }), this.declareByteCodeHarInputFiles(i), this.declareDynamicImportInputFiles(i), this.targetService.shouldDoDeduplicateTask() && this.moduleModel.isHapModule() && this.addHarDeduplicateInputFiles(i), i;
  }
  addStartupInputFiles(e) {
    this.appStartupPath && fse.existsSync(this.appStartupPath) && e.addEntry(this.appStartupPath), this.intermediateTempStartupFilePath && fse.existsSync(this.intermediateTempStartupFilePath) && e.addEntry(this.intermediateTempStartupFilePath);
  }
  isFullCompilationEnabled() {
    const e = hvigor_1.coreParameter.properties[common_const_js_1.HvigorConfigConst.OHOS_COMPILE_LIB_ENTRY];
    return (!this.moduleModel.isFormWidgetModule(this.targetName) || undefined !== e) && !e;
  }
  getEtsSourcePath(e) {
    return path_1.join(e, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, code_type_enum_js_1.CodeType.ETS);
  }
  get isBundledDependencies() {
    return this.targetService.isBundledDependencies();
  }
  declareDynamicImportInputFiles(e) {
    var t, i, s;
    if (!this.allowToCollectDependencyEtsFolder()) return;
    const o = [],
      n = new Map();
    this.service.getHarDependencies().forEach(e => {
      n.set(e.getDependencyName(), e);
      const t = e.isLocal() ? this.getLocalDependencyDynamicImportList(e) : this.getRemoteDependencyDynamicImportList(e);
      o.push(...t);
    });
    const r = null === (s = null === (i = null === (t = this.targetService.getBuildOption()) || undefined === t ? undefined : t.arkOptions) || undefined === i ? undefined : i.runtimeOnly) || undefined === s ? undefined : s.packages;
    o.concat(r || []).forEach(t => {
      const i = n.get(t);
      if (!i) return;
      const s = this.getEtsSourcePath(i.getDependencyRootPath());
      fse.existsSync(s) && e.addEntry(s, {
        isDirectory: true
      });
    });
  }
  declareByteCodeHarInputFiles(e) {
    const t = this.pathInfo.getGenerateModuleInfoPath();
    this.targetService.isByteCodeHar() && fse.existsSync(t) && e.addEntry(t, {
      isDirectory: false
    }), Object.keys(this.byteCodeHarInfo).forEach(t => {
      const {
        abcPath: i,
        sourceMapsPath: s
      } = this.byteCodeHarInfo[t];
      e.addEntry(i, {
        isDirectory: false
      }), s && e.addEntry(i, {
        isDirectory: false
      });
    }), this.otherCompileFiles.forEach(t => {
      e.addEntry(t, {
        isDirectory: false
      });
    });
  }
  addHarDeduplicateInputFiles(e) {
    this.allHspDepTargets && this.allHspDepTargets.forEach((t, i) => {
      const s = t.getPathInfo().getSourceHarDuplicateFilesInfoPath();
      fse.existsSync(s) && e.addEntry(s);
      const o = t.getPathInfo().getByteCodeHarDuplicateFilesInfoPath();
      fse.existsSync(o) && e.addEntry(o);
      const n = t.getPathInfo().getHarDuplicateInfoPath();
      fse.existsSync(n) && e.addEntry(n);
    });
  }
  declareOutputFiles() {
    const e = new hvigor_1.FileSet().addEntry(this.aceLoaderJson, {
      isDirectory: false
    });
    return this.needSubmitArkTsWidget && e.addEntry(this.aceWidgetLoaderJson, {
      isDirectory: false
    }), this.targetService.shouldDoDeduplicateTask() && this.moduleModel.isHspModule() && e.addEntry(this.pathInfo.getHarDuplicateInfoPath()), e;
  }
  initTaskDepends() {
    this.declareDepends(pre_build_js_1.PreBuild.name);
  }
  constructor(e) {
    super(e, task_names_js_1.TaskNames.Task.GENERATE_LOADER_JSON), this.hspNameOhmMap = {}, this.harNameOhmMap = {}, this.otherCompileFiles = [], this.updateVersionInfo = {}, this.declarationEntry = [], this.pkgContextInfos = {}, this.byteCodeHarInfo = {}, this.aceLoaderJson = this.pathInfo.getLoaderJsonPath(), this.aceWidgetLoaderJson = this.pathInfo.getWidgetLoaderJsonPath(), this.nodeModulesPath = path_1.resolve(e.getTargetData().getPathInfo().getInterMediatesLoaderOutPath(), common_const_js_1.CommonNodeConst.NODE_MODULES), this.hspResourcesMap = this.getHspResourcesMap(), this.moduleSrcMainPath = e.getTargetData().getPathInfo().getModuleSrcMainPath(), this.modulePath = e.getTargetData().getPathInfo().getModulePath(), this.moduleSrcMockPath = e.getTargetData().getPathInfo().getModuleSrcMockPath(), this.mockConfigJsonPath = path_1.resolve(this.moduleSrcMockPath, build_directory_const_js_1.BuildArtifactConst.MOCK_CONFIG_JSON5), this.moduleJson5Path = path_1.resolve(this.moduleSrcMainPath, build_directory_const_js_1.BuildArtifactConst.MODULE_JSON5), this.etsSourcePath = path_1.join(this.pathInfo.getModuleSrcMainPath(), code_type_enum_js_1.CodeType.ETS), this.insightIntentJsonPath = e.getInsightIntentJsonPath(), this.compileEntry = this.readInsightIntentSrcEntry(), this.dynamicImportLibInfo = {}, this.intermediatesRouterMapObjs = [], this.tempApDir = this.pathInfo.getIntermediatesApPath(), this.tempApPath = path_1.resolve(this.tempApDir, `${this.moduleName}.ap`), this.anBuildOutPutPath = path_1.resolve(this.pathInfo.getInterMediatesLoaderOutPath(), aot_compile_mode_enum_js_1.AotCompileModeEnum.AN, aot_compile_mode_enum_js_1.AotCompileModeEnum.ARM64_V8A), this.isPreview = hvigor_1.hvigorCore.getExtraConfig().get(inject_const_js_1.InjectConst.BUILD_ROOT) === build_directory_const_js_1.BuildDirConst.PREVIEW_BUILD_PATH, this.fallbackAnBuild = !inject_util_js_1.InjectUtil.isTestFrameWork() && this.moduleModel.isHarModule();
    const t = this.targetService.getApPath();
    if (t && (this.apPath = path_1.resolve(this.moduleModel.getProjectDir(), t)), this.needSubmitArkTsWidget = (0, task_util_js_1.shouldCompileArkTsWidget)(target_task_service_js_1.TargetTaskService.getFormJsonArr(e), this.moduleModel.isFormExtensionModule(this.targetName)), this.targetService.shouldDoDeduplicateTask() && this.moduleModel.isHapModule()) {
      const e = (0, collect_duplicate_har_dependencies_js_1.getMergedHapAllHspDependencies)();
      this.allHspDepTargets = hsp_target_utils_js_1.HspTargetUtils.calHspTargets(this.targetService, e);
    }
    this.ohExportsFilePaths = this.getOhExportsFilePaths();
  }
  async beforeAlwaysAction() {
    await super.beforeAlwaysAction(), this.setPkgContextInfos(), this.hspNameOhmMap = this.getHspNameOhmMap(), this.harNameOhmMap = this.getHarNameOhmMap(), this.setByteCodeHarInfoAndOtherCompileFiles(), this.targetService.shouldDoDeduplicateTask() && this.processByteCodeHarInfoAndOtherCompileFilesDeduplicate(), this.checkOhExports();
  }
  isHapOrHsp() {
    return this.moduleModel.isHspModule() || this.moduleModel.isHapModule() || this.shouldTreatHarAsHap();
  }
  setByteCodeHarInfoAndOtherCompileFiles() {
    if (!this.useNormalizedOHMUrl || !this.isHapOrHsp()) return;
    const {
      byteCodeInfo: e,
      otherCompileEntrances: t
    } = (0, byte_code_har_utils_js_1.collectByteCodeInfoAndOtherCompileEntrances)(this.targetService);
    e.forEach(e => {
      const t = e.packageName;
      this.byteCodeHarInfo[t] = {
        abcPath: e.abcPath,
        compatibleSdkVersion: e.compatibleSdkVersion,
        compatibleSdkVersionStage: e.compatibleSdkVersionStage,
        ...(e.sourceMapsPath ? {
          sourceMapsPath: e.sourceMapsPath
        } : {}),
        ...(e.insightIntentJsonPath ? {
          insightIntent: e.insightIntentJsonPath
        } : {})
      };
    }), t.forEach((e, t) => {
      e ? this.collectCompileFiles(t, this.otherCompileFiles) : this.otherCompileFiles.push(t);
    });
  }
  processByteCodeHarInfoAndOtherCompileFilesDeduplicate() {
    this.moduleModel.isHapModule() ? this.addDuplicateByteCodeHarInfoAndOtherCompileFiles() : this.moduleModel.isHspModule() && this.collectHspDuplicateByteCodeHarInfoAndOtherCompileFiles();
  }
  beforeTask() {
    super.beforeTask(), fse.existsSync(this.aceWidgetLoaderJson) && fse.removeSync(this.aceWidgetLoaderJson);
  }
  getWorkerConfig() {
    var e;
    if (this.moduleModel.isHarModule()) return 0 === this.targetService.getWorkerConfig().length ? undefined : this.targetService.getWorkerConfig();
    const t = this.service.getHarModuleDependencies().reduce((e, t) => {
        var i, s, o;
        const n = this.projectModel.getModuleModelByName(t[0]);
        if (undefined === n) return e;
        const r = null !== (i = har_target_util_js_1.HarTargetUtil.calDepHarTargets(this.targetService).get(t[0])) && undefined !== i ? i : "default",
          a = build_mode_manager_js_1.buildOptionManager.getTargetBuildOption(t[0], r),
          l = file_util_js_1.FileUtil.convertToAbsolutePaths(null !== (o = null === (s = a.sourceOption) || undefined === s ? undefined : s.workers) && undefined !== o ? o : [], n.getProjectDir());
        return l ? e.concat(l) : e;
      }, null !== (e = this.targetService.getWorkerConfig()) && undefined !== e ? e : []),
      i = this.service.getHarModuleDependencyPaths();
    return this.service.getHarDependencies().forEach(e => {
      var s;
      if (i.includes(e.getDependencyRootPath())) return;
      const o = json_util_js_1.JsonUtil.getJson5Obj(e.getPackageFilePath(), "utf-8");
      (null === (s = null == o ? undefined : o.metadata) || undefined === s ? undefined : s.workers) && t.push(...file_util_js_1.FileUtil.convertToAbsolutePaths(o.metadata.workers, e.getDependencyRootPath()));
    }), 0 === t.length ? undefined : t;
  }
  collectCompileFiles(e, t = this.compileEntry) {
    if (!fse.existsSync(e)) return;
    const i = [".ets", ".ts", ".js"],
      s = [e];
    for (; s.length;) {
      const e = s.shift();
      fse.readdirSync(e).forEach(o => {
        const n = path_1.join(e, o),
          r = fse.statSync(n);
        r.isDirectory() ? s.push(n) : r.isFile() && i.includes(path_1.extname(n).toLowerCase()) && t.push(n);
      });
    }
  }
  allowToCollectEtsFolder() {
    return !(this.targetName === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET || !this.isFullCompilationEnabled()) && (this.moduleModel.isHspModule() || this.moduleModel.isHarModule());
  }
  allowToCollectDependencyEtsFolder() {
    return !!this.isFullCompilationEnabled() && (this.moduleModel.isHspModule() || this.moduleModel.isHapModule());
  }
  collectDepEtsFolderFilesEntry(e) {
    this.allowToCollectDependencyEtsFolder() && this.collectCompileFiles(this.getEtsSourcePath(e.getDependencyRootPath()));
  }
  checkByteCodeHar() {
    if (this.useNormalizedOHMUrl || !this.isHapOrHsp()) return;
    const e = this.service.getHarDependencies().filter(e => e.isByteCodeHarDependency()).map(e => e.getPackageName());
    e.length && this._log.printErrorExit("BYTECODE_HARS_%S_NOT_SUPPORTED_WHEN_USENORMALIZEDOHMURL_IS_NOT_TRUE", [e.join(", ")]);
  }
  shouldTreatHarAsHap() {
    return this.moduleModel.isHarModule() && (0, byte_code_har_utils_js_1.shouldTreatHarAsHap)(this.targetName);
  }
  isHarWithCoverage() {
    return this.moduleModel.isHarModule() && inject_util_js_1.InjectUtil.isOhosTestCoverage();
  }
  getModuleEntryFile() {
    const e = this.packageJsonObj.main ? this.packageJsonObj.main : build_directory_const_js_1.BuildArtifactConst.INDEX_ETS;
    return path_1.resolve(this.pathInfo.getModulePath(), e);
  }
  processHarUnitTestWithCoverage() {
    const e = this.getModuleEntryFile();
    this.isHarWithCoverage() && fse.existsSync(e) && this.compileEntry.push(e);
  }
  processCodeFiles(e) {
    const t = /\.d\.(e)?ts$/i;
    return Array.from(new Set(e)).filter(e => !t.test(e) && fse.existsSync(e)).map(e => fse.realpathSync.native(e));
  }
  setPkgContextInfos() {
    fse.existsSync(this.pathInfo.getIntermediatesPkgContextInfoPath()) && (this.pkgContextInfos = json_util_js_1.JsonUtil.getJson5Obj(this.pathInfo.getIntermediatesPkgContextInfoPath()));
  }
  doTaskAction() {
    var e, t, i, s, o, n;
    this.checkByteCodeHar();
    const r = this.moduleModel.getSourceSetByTargetName(this.targetName);
    if (this.targetName === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET) {
      const s = null === (i = null === (t = null === (e = r.getModuleTargetRes()) || undefined === e ? undefined : e.getModuleJsonOpt()) || undefined === t ? undefined : t.module) || undefined === i ? undefined : i.name;
      s && (this.modulePathMap = {
        [s]: this.module.getNodeDir(),
        ...this.modulePathMap
      });
    }
    this.processHarUnitTestWithCoverage();
    const a = this.targetName === common_const_js_1.CommonConst.OHOS_TEST,
      l = inject_util_js_1.InjectUtil.isLocalTest();
    (this.isPreview || a || l) && this.collectMockFileEntry(this.modulePath, this.mockConfigJsonPath), this.allowToCollectEtsFolder() && this.collectCompileFiles(this.etsSourcePath), fse.existsSync(this.insightIntentJsonPath) && this.validateForInsightIntentJson(), this.processDeclarationEntry(), this.processDynamicImport(), this.processByteCodeHar(), this.processAppStartupConfig(this.appStartupPath), this.processRouterMap(), this.processBundledDependencies(), this.processUpdateVersionInfo(), this.collectDepAbilitySrcEntryArr(), this.addHarDepInsightSrcEntry(), this.targetService.shouldDoDeduplicateTask() && this.processHarDeduplicate(), this.shouldProcessOhExports() && this.addOhExports2CompileEntry(this.ohExportsFilePaths);
    const d = this.handleAotFields(),
      c = {
        workers: this.workerConfig,
        modulePathMap: this.modulePathMap,
        compileMode: this.targetCompileMode,
        projectRootPath: this.projectRootPath,
        nodeModulesPath: this.nodeModulesPath,
        byteCodeHarInfo: this.byteCodeHarInfo,
        declarationEntry: (0, wdk_1.uniq)(this.declarationEntry),
        moduleName: null === (n = null === (o = null === (s = r.getModuleTargetRes()) || undefined === s ? undefined : s.getModuleJsonOpt()) || undefined === o ? undefined : o.module) || undefined === n ? undefined : n.name,
        hspNameOhmMap: this.hspNameOhmMap,
        harNameOhmMap: this.harNameOhmMap,
        packageManagerType: this.isOhpmProject ? "ohpm" : "npm",
        compileEntry: this.processCodeFiles(this.compileEntry),
        otherCompileFiles: this.processCodeFiles(this.otherCompileFiles),
        dynamicImportLibInfo: this.dynamicImportLibInfo,
        routerMap: this.intermediatesRouterMapObjs,
        hspResourcesMap: this.hspResourcesMap,
        updateVersionInfo: this.updateVersionInfo,
        customizedHar: this.targetService.isCustomizedHar(),
        ...d
      };
    this.addResourceTable2CompileEntry(c), this.targetService.isByteCodeHar() && (c.byteCodeHar = true);
    const h = this.pathInfo.getBuildConfigPath();
    if ((this.isPreview || inject_util_js_1.InjectUtil.isLocalTest()) && (c.buildConfigPath = path_1.relative(this.modulePath, h)), fse.existsSync(h) && !inject_util_js_1.InjectUtil.isOhosTest()) {
      const e = json_util_js_1.JsonUtil.getJson5Obj(h);
      c.patchConfig = e.patchConfig;
    }
    this.shouldProcessOhExports() && this.writeOhExportsToLoaderJson(this.ohExportsFilePaths, c), fse.outputJSONSync(this.aceLoaderJson, c, {
      spaces: 2
    }), this.needSubmitArkTsWidget && fse.outputJSONSync(this.aceWidgetLoaderJson, c, {
      spaces: 2
    });
  }
  processByteCodeHar() {
    const e = this.pathInfo.getGenerateModuleInfoPath();
    this.targetService.isByteCodeHar() && fse.existsSync(e) && this.compileEntry.push(e);
  }
  collectByteCodeHarDeclarationEntry(e, t) {
    var i, s, o;
    const n = e.getPackageJsonObj();
    if (e.isByteCodeHarDependency()) {
      const e = (null === (i = n.metadata) || undefined === i ? undefined : i.declarationEntry) || [];
      this.declarationEntry.push(...e);
    }
    const r = null === (o = null === (s = n.metadata) || undefined === s ? undefined : s.runtimeOnly) || undefined === o ? undefined : o.packages;
    this.collectRuntimeDynamicImportByteCodeHarDeclarationEntry(t, r);
  }
  collectRuntimeDynamicImportByteCodeHarDeclarationEntry(e, t) {
    (null == t ? undefined : t.length) && t.forEach(t => {
      const i = e.get(t);
      if (!(null == i ? undefined : i.isByteCodeHarDependency())) return;
      const s = this.pkgContextInfos[i.getPackageName()];
      s ? this.declarationEntry.push((0, ohmurl_utils_js_1.generateOhmUrl)(s)) : this._log.debug(`Can not find package context information with package name: ${i.getPackageName()}`);
    });
  }
  shouldCollectDeclarationEntry() {
    return !!this.useNormalizedOHMUrl && this.isHapOrHsp();
  }
  processDeclarationEntry() {
    var e, t;
    if (!this.shouldCollectDeclarationEntry()) return;
    const i = this.service.getHarDependencies(),
      s = i.reduce((e, t) => (e.set(t.getPackageName(), t), e), new Map());
    i.forEach(e => {
      this.collectByteCodeHarDeclarationEntry(e, s);
    });
    const o = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.runtimeOnly;
    this.collectRuntimeDynamicImportByteCodeHarDeclarationEntry(s, null == o ? undefined : o.packages);
  }
  collectMockFileEntry(e, t) {
    if (!fse.existsSync(t)) return;
    const i = hvigor_1.ParseJsonFile.parseJsonFile(t, false);
    i && Object.values(i).forEach(t => {
      const i = t.source;
      if (i) {
        const t = path_1.resolve(e, i);
        fse.existsSync(t) && this.compileEntry.push(t);
      }
    });
  }
  extensionAbilitiesValidate(e, t, i, s) {
    if (undefined === e.form || undefined === e.form.ability) return;
    const o = e.form.ability;
    t.has(o) && "form" === t.get(o).type || log.printErrorExit("INSIGHT_INTENT_INVALID_FORM_ABILITY_NAME", [o, this.insightIntentJsonPath], [[o]]);
  }
  intentFormNameValidate(e, t) {
    var i;
    const s = null === (i = e.form) || undefined === i ? undefined : i.formName;
    undefined !== s && (t.length && t.includes(s) || this._log.printErrorExit("INVALID_FORM_NAME_%S", [s]));
  }
  validateServiceExtensionAbility(e, t, i, s, o) {
    var n;
    if (undefined !== e.uiAbility && undefined !== e.uiAbility.ability) {
      const i = e.uiAbility.ability;
      undefined !== i && (t.has(i) || log.printErrorExit("INSIGHT_INTENT_INVALID_ABILITY_NAME", [i, this.insightIntentJsonPath], [[i]]));
    }
    if (undefined === e.serviceExtension || undefined === e.serviceExtension.ability) return;
    const r = e.serviceExtension.ability;
    i.has(r) && "service" === (null === (n = i.get(r)) || undefined === n ? undefined : n.type) || log.printErrorExit("INSIGHT_INTENT_INVALID_SERVICE_EXTENSION_ABILITY_NAME", [r, this.insightIntentJsonPath], [[r]]);
  }
  uiExtensionValidate(e, t, i, s, o) {
    if (undefined === e.uiExtension || undefined === e.uiExtension.ability) return;
    const n = e.uiExtension.ability;
    t.has(n) || i.has(n) || log.printErrorExit("INSIGHT_INTENT_INVALID_UI_EXTENSION_ABILITY_NAME", [n, this.insightIntentJsonPath], [[n]]);
  }
  validateForInsightIntentJson() {
    var e;
    let t = "",
      i = "";
    for (const e of this.compileEntry) fse.existsSync(e) || (t = `Cannot find the file under the specified path '${e}'.`, i = "Please check if the file in the path exists.", log.printErrorExit("FILE_NOT_EXIST", [e])), e.endsWith(".ets") || (t = `The file must end with '.ets' in '${e}'.`, i = "Please check if the file name is correct.", log.printErrorExit("FILE_NO_END_WITH_ETS", [e, this.insightIntentJsonPath]));
    const s = new Set(),
      o = hvigor_1.ParseJsonFile.parseJsonFile(this.moduleJson5Path, false);
    if (undefined !== o.module.abilities) for (const e of o.module.abilities) s.add(e.name);
    const n = new Map();
    if (undefined !== o.module.extensionAbilities) for (const e of o.module.extensionAbilities) n.set(e.name, e);
    if (null !== this.insightIntentJson) {
      const o = this.getFormConfigNames();
      for (const r of null !== (e = this.insightIntentJson.insightIntents) && undefined !== e ? e : []) this.validateServiceExtensionAbility(r, s, n, t, i), this.extensionAbilitiesValidate(r, n, t, i), this.intentFormNameValidate(r, o), this.uiExtensionValidate(r, s, n, t, i);
    }
  }
  getFormConfigNames() {
    var e, t;
    const i = path_1.resolve(this.pathInfo.getModuleSrcMainResourceBaseProfilePath(), build_directory_const_js_1.BuildArtifactConst.FORM_CONFIG_JSON);
    if (fse.existsSync(i)) {
      const s = hvigor_1.Json5Reader.getJson5Obj(i);
      return null !== (t = null === (e = null == s ? undefined : s.forms) || undefined === e ? undefined : e.map(e => e.name)) && undefined !== t ? t : [];
    }
    return [];
  }
  readInsightIntentSrcEntry() {
    var e, t;
    const i = new Set();
    let s = "";
    if (fse.existsSync(this.insightIntentJsonPath)) {
      const o = hvigor_1.ParseJsonFile.parseJsonFile(this.insightIntentJsonPath, false);
      this.insightIntentJson = o;
      const n = this.moduleSrcMainPath;
      for (const r of [...(null !== (e = o.insightIntents) && undefined !== e ? e : []), ...(null !== (t = o.insightIntentsSrcEntry) && undefined !== t ? t : [])]) {
        const e = r.srcEntry;
        s = path_1.resolve(n, e), i.add(s);
      }
    }
    return Array.from(i);
  }
  checkAotCompatibleSdkVersion() {
    if (!this.sdkInfo.isOhos) return;
    const e = sdk_version_enum_js_1.SdkVersionEnum.SUPPORT_AOT_OHOS,
      t = this.sdkInfo.getEtsComponentVersion();
    if ("" === t) return;
    const i = new sdk_version_js_1.SdkVersion(t);
    if (e.isHigherThan(i)) {
      const t = "OpenHarmony SDK";
      this._log.printErrorExit("AOT_COMPILING_UNSUPPORTED_SDK_VERSION", [i.getVersion(), e.getVersion()], [[t]]);
    }
  }
  handleAotFields() {
    const e = this.targetService.getAnBuildMode();
    this.warnAnBuildMode(), e && this.compileApiVersion === sdk_const_js_1.ApiVersion.API_VERSION_9 && this.checkAotCompatibleSdkVersion();
    const t = {
      anBuildMode: e,
      apPath: this.apPath,
      apiVersion: this.compileApiVersion,
      fallbackAnBuild: this.fallbackAnBuild
    };
    return this.getAotStrategy(t);
  }
  warnAnBuildMode() {
    var e, t, i, s, o;
    const n = this.targetData.getCompileApiVersion(),
      r = null !== (i = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.aotCompileMode) && undefined !== i ? i : null === (s = this.targetService.getBuildOption()) || undefined === s ? undefined : s.aotCompileMode,
      a = null === (o = this.targetService.getBuildOption().arkOptions) || undefined === o ? undefined : o.hostPGO;
    switch (n) {
      case sdk_const_js_1.ApiVersion.API_VERSION_9:
        undefined !== a && log.warn('Field "hostPGO" is not supported in API version 9. The value of this field will be ignored.');
        break;
      case sdk_const_js_1.ApiVersion.API_VERSION_10:
      default:
        if (undefined !== r && log.warn('Field "aotCompileMode" is not supported since API version 10. The value of this field will be ignored.'), this.sdkInfo.isOhos && !this.sdkInfo.checkAotFixedSdkVersion()) {
          const e = "SDK > OpenHarmony";
          log._buildError(`Current version of sdk has a bug when using AoT compiling. The compiling will continue with AoT compiling function disabled. Current SDK version is ${this.sdkInfo.getEtsComponentVersion()}, but the lowest fixed SDK version is ${sdk_version_enum_js_1.SdkVersionEnum.FIXED_AOT_OHOS.getVersion()}.`)._detail(`Update SDK to the latest version by going to Tools > SDK Manager > ${e}. Open SDK Manager`)._printWarn(this.moduleName);
        }
    }
  }
  getHarNameOhmMap() {
    if (!this.targetService.isByteCodeHar()) return {};
    if (this.targetService.isBundledDependencies()) {
      const e = this.isByteCodeHarOptimize ? this.service.getRootOtherDependencies().concat(this.service.getRootByteCodeHarDependencies()) : this.service.getOtherDependencies().concat(this.service.getHarDependencies().filter(e => e.isByteCodeHarDependency()));
      return this.getOHMUrlMap(e, this.pkgContextInfos);
    }
    const e = (this.isByteCodeHarOptimize ? this.service.getAllDirectDependencies() : this.service.getDirectDependencies()).filter(e => e.getDependencyType() !== dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP);
    return this.getOHMUrlMap(e, this.pkgContextInfos);
  }
  getHspDependencies() {
    const e = inject_util_js_1.InjectUtil.isPreviewProcess() || inject_util_js_1.InjectUtil.isLocalTest();
    if (this.isByteCodeHarOptimize) return e ? this.service.getAllHspDependencies() : this.service.getRootHspDependencies();
    if (e) {
      const e = new dependency_manager_js_1.DependencyManager(this.projectModel.isFaMode(), this.moduleModel, this.projectModel),
        {
          npmDependencies: t
        } = e.collectAllDependencies();
      return t.filter(e => e.getDependencyType() === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP);
    }
    return this.service.getHspDependencies();
  }
  getHspNameOhmMap() {
    let e = {};
    const t = this.getHspDependencies();
    return this.useNormalizedOHMUrl ? e = this.getOHMUrlMap(t, this.pkgContextInfos) : t.forEach(t => {
      const i = t.getDependencyName(),
        s = this.getHspDependencyModuleName(t),
        o = this.getHspDependencyPackageMainName(t);
      "" !== o && (e[i] = `@bundle:${s}${o}`);
    }), e;
  }
  getHspResourcesMap() {
    const e = {};
    return hsp_target_utils_js_1.HspTargetUtils.calDepHspTargets(this.targetService).forEach((t, i) => {
      const s = path_1.resolve(t.getPathInfo().getIntermediatesRes(), build_directory_const_js_1.BuildArtifactConst.RESOURCE_TABLE_TXT);
      fse.existsSync(s) ? e[t.getModuleModel().getName()] = s : this._log.debug(`The ResourceTable.txt in ${t.getPathInfo().getModulePath()} does not exist.`);
    }), this.service.getHspDependencies().forEach(t => {
      const i = t.getDependencyName();
      let s;
      t.isLocal() || (s = path_1.resolve(t.getDependencyRootPath(), build_directory_const_js_1.BuildArtifactConst.RESOURCE_TABLE_TXT)), s && fse.existsSync(s) ? e[i] = s : this._log.debug(`The ResourceTable.txt in ${t.getDependencyRootPath()} does not exist.`);
    }), e;
  }
  getOHMUrlMap(e, t) {
    const i = {},
      s = e.map(e => e.getPackageName());
    return Object.values(t).forEach(t => {
      if (s.includes(t.packageName)) {
        const s = e.filter(e => e.getPackageName() === t.packageName);
        i[s[0].getDependencyName()] = (0, ohmurl_utils_js_1.generateOhmUrl)(t);
      }
    }), i;
  }
  getHspDependencyPackageMainName(e) {
    const t = path_1.resolve(e.getDependencyRootPath(), "./src/main");
    let i, s;
    return i = hvigor_1.Json5Reader.getJson5Obj(e.getPackageFilePath()).main ? e.getDependencyMainFilePath() : e.getDependencyTypesFilePath(), i.startsWith(t) ? s = i.replace(t, "") : i.startsWith(e.getDependencyRootPath()) && (s = i.replace(e.getDependencyRootPath(), "")), s ? (0, ohmurl_utils_js_1.removeSuffix)(s) : (log._buildError(`The "main" or "types" field of hsp module ${e.getPackageName()} must be under the\n      root path of hsp module or under src/main path. Please confirm.`)._file(e.getPackageFilePath())._printWarn(this.moduleName), "");
  }
  getHspDependencyModuleName(e) {
    var t, i, s, o, n;
    const r = this.service.getHspModuleDependencyPaths();
    let a;
    const l = path_1.resolve(e.getDependencyRootPath(), "src", "main", "module.json5"),
      d = path_1.resolve(e.getDependencyRootPath(), "src", "main", "module.json");
    let c;
    const h = (0, wdk_1.cloneDeep)(null === (t = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === t ? undefined : t.appOpt);
    return r.includes(e.getDependencyRootPath()) ? (a = hvigor_1.Json5Reader.getJson5Obj(l), c = null !== (s = null !== (i = null == h ? undefined : h.bundleName) && undefined !== i ? i : this.targetData.getProduct().bundleName) && undefined !== s ? s : this.projectModel.getDefaultBundleName()) : fse.existsSync(d) ? (a = json_util_js_1.JsonUtil.getJson5Obj(d), c = a.app.bundleName) : (a = hvigor_1.Json5Reader.getJson5Obj(l), c = null !== (n = null !== (o = null == h ? undefined : h.bundleName) && undefined !== o ? o : this.targetData.getProduct().bundleName) && undefined !== n ? n : this.projectModel.getDefaultBundleName()), `${c}/${a.module.name}`;
  }
  getAotStrategy(e) {
    const {
        apiVersion: t,
        anBuildMode: i,
        apPath: s,
        fallbackAnBuild: o
      } = e,
      n = undefined !== s && fse.pathExistsSync(s),
      r = undefined !== s && s.endsWith("ap"),
      a = [{
        name: "withEmpty",
        condition: o,
        strategy: this.withEmpty
      }, {
        name: "mismatchAotCompileMode",
        condition: t === sdk_const_js_1.ApiVersion.API_VERSION_9 && i !== aot_compile_mode_enum_js_1.AotCompileModeEnum.AOT_PARTIAL && undefined !== s,
        strategy: this.mismatchAotCompileMode
      }, {
        name: "partialEmptyPath",
        condition: t === sdk_const_js_1.ApiVersion.API_VERSION_9 && i === aot_compile_mode_enum_js_1.AotCompileModeEnum.AOT_PARTIAL && undefined === s,
        strategy: this.partialEmptyPath
      }, {
        name: "invalidSuffix",
        condition: undefined !== s && !r,
        strategy: this.invalidSuffix
      }, {
        name: "invalidPath",
        condition: i === aot_compile_mode_enum_js_1.AotCompileModeEnum.AOT_PARTIAL && !n,
        strategy: this.invalidPath
      }, {
        name: "withEmpty",
        condition: (t === sdk_const_js_1.ApiVersion.API_VERSION_9 && undefined === s || t >= sdk_const_js_1.ApiVersion.API_VERSION_10) && i === aot_compile_mode_enum_js_1.AotCompileModeEnum.AOT_NULL,
        strategy: this.withEmpty
      }, {
        name: "withApPath",
        condition: (t === sdk_const_js_1.ApiVersion.API_VERSION_9 && i === aot_compile_mode_enum_js_1.AotCompileModeEnum.AOT_PARTIAL || t >= sdk_const_js_1.ApiVersion.API_VERSION_10) && n && r,
        strategy: this.withApPath
      }, {
        name: "withoutApPath",
        condition: t === sdk_const_js_1.ApiVersion.API_VERSION_9 && i === aot_compile_mode_enum_js_1.AotCompileModeEnum.AOT_TYPE && undefined === s,
        strategy: this.withoutApPath
      }, {
        name: "withoutApPath",
        condition: t >= sdk_const_js_1.ApiVersion.API_VERSION_10 && i === aot_compile_mode_enum_js_1.AotCompileModeEnum.AOT_TYPE && !n,
        strategy: this.withoutApPath
      }];
    return (0, strategy_js_1.getStrategy)(a, () => ({})).bind(this)(e);
  }
  withApPath(e) {
    return fse.emptyDirSync(this.tempApDir), fse.copyFileSync(e.apPath, this.tempApPath), {
      anBuildOutPut: this.anBuildOutPutPath,
      anBuildMode: e.anBuildMode,
      apPath: e.apPath
    };
  }
  withoutApPath(e) {
    return {
      anBuildOutPut: this.anBuildOutPutPath,
      anBuildMode: e.anBuildMode
    };
  }
  withEmpty() {
    return {};
  }
  mismatchAotCompileMode() {
    this._log.printErrorExit("MISMATCH_AOT_COMPILE_MODE", [this.moduleModel.getProfilePath()]);
  }
  partialEmptyPath() {
    this._log.printErrorExit("PARTIAL_EMPTY_PATH", [this.moduleModel.getProfilePath()]);
  }
  invalidPath() {
    this._log.printErrorExit("PARTIAL_INVALID_PATH", [this.moduleModel.getProfilePath(), this.apPath]);
  }
  invalidSuffix() {
    this._log.printErrorExit("PARTIAL_INVALID_SUFFIX", [build_option_path_info_js_1.buildOptionPath.getTargetBuildOptPath(this.moduleModel, this.targetName, "apPath")]);
  }
  processAppStartupConfig(e) {
    var t;
    if (undefined !== e && fse.existsSync(e)) {
      const t = json_util_js_1.JsonUtil.getJson5Obj(e),
        i = path_1.resolve(this.module.getNodeDir(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN);
      t.configEntry && this.compileEntry.push(path_1.resolve(i, t.configEntry));
    }
    const i = json_util_js_1.JsonUtil.getJson5Obj(this.intermediateTempStartupFilePath);
    undefined !== i && (null === (t = i.startupTasks) || undefined === t || t.forEach(e => {
      undefined !== e.srcEntryAbsolutePath && this.compileEntry.push(null == e ? undefined : e.srcEntryAbsolutePath);
    }));
  }
  processDynamicImport() {
    var e, t;
    this.targetService.isByteCodeHar() || this.processIndirectDependency(this.service.getHarDependencies());
    const i = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.runtimeOnly;
    if (i) {
      if (i.sources) {
        const e = this.module.getNodeDir();
        i.sources.forEach(t => {
          const i = path_1.resolve(e, t),
            s = fse.statSync(i);
          s.isFile() && this.compileEntry.push(i), s.isDirectory() && this.processDirectoryImport(i);
        });
      }
      i.packages && !this.targetService.isByteCodeHar() && i.packages.forEach(e => {
        this.isHarDependency(e) ? this.processHarDynamicImport(e, this.dynamicImportLibInfo) : this.processOtherDynamicImport(e, this.dynamicImportLibInfo);
      });
    }
  }
  processHarDynamicImport(e, t) {
    this.service.getHarDependencies().forEach(i => {
      var s, o;
      const n = i.getDependencyName();
      if (n === e) {
        const r = i.isByteCodeHarDependency() ? i.getDependencyTypesFilePath() : i.getDependencyMainFilePath(),
          a = i.getPackageFilePath();
        if (this.existInDynamicImportLibsInfo(e)) return;
        const l = {
          [n]: {
            hostModulesInfo: [{
              hostDependencyName: n,
              hostModuleName: this.moduleModel.getName()
            }],
            moduleName: this.getHarDependencyModuleName(i, e),
            entryFilePath: r,
            isLocalDependency: i.isLocal(),
            pkgPath: a.substring(0, a.lastIndexOf(path_1.sep))
          }
        };
        if (Object.assign(t, l), i.isByteCodeHarDependency()) return;
        const d = null === (o = null === (s = this.targetService.getBuildOption()) || undefined === s ? undefined : s.arkOptions) || undefined === o ? undefined : o.runtimeOnly,
          c = new Set((0, exclude_remote_dynamic_import_js_1.collectFromBuildOption)(null != d ? d : {}));
        if ((0, exclude_remote_dynamic_import_js_1.isNeedToExcludeHar)(c, this.service.getRemoteHarDependencies(), e)) return;
        this.compileEntry.push(r), this.collectDepEtsFolderFilesEntry(i);
      }
    });
  }
  processOtherDynamicImport(e, t) {
    const i = path_1.resolve(this.moduleModel.getModule().getNodeDir(), common_const_js_1.CommonConst.OH_MODULES, e),
      s = path_1.resolve(this.projectRootPath, common_const_js_1.CommonConst.OH_MODULES, e),
      o = fse.existsSync(i) ? i : fse.existsSync(s) ? s : "";
    if ("" === o) return;
    const n = fse.realpathSync(o);
    if (!fse.existsSync(n)) return;
    const r = oh_package_loader_js_1.ohPackageLoader.getNodeOhPackagePath(path_1.resolve(o, common_const_js_1.CommonConst.OH_PACKAGE_JSON5)),
      a = json_util_js_1.JsonUtil.getJson5Obj(r);
    if (!a) return;
    const l = this.getEntryMainFilePath(n, a),
      d = path_1.resolve(n, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN),
      c = dependency_manager_js_1.DependencyManager.isLocalDependencyForSo(l),
      h = c ? path_1.resolve(d, common_const_js_1.CommonConst.MODULE_JSON5) : path_1.resolve(d, common_const_js_1.CommonConst.MODULE_JSON);
    let u,
      p = a.name;
    if (fse.existsSync(h)) {
      const t = json_util_js_1.JsonUtil.getJson5Obj(h);
      if (!t) return;
      u = t.module.type, this.isHspDependency(e) && (p = t.module.name);
    }
    if (u || u === module_type_enum_js_1.ModuleType.Shared || common_const_js_1.ValidateRegExp.SO_DEPENDENCY_REG_EXP.test(e) || this.compileEntry.push(l), this.existInDynamicImportLibsInfo(e)) return;
    const _ = {
      [e]: {
        hostModulesInfo: [{
          hostDependencyName: e,
          hostModuleName: this.moduleModel.getName()
        }],
        moduleName: p,
        entryFilePath: l,
        isLocalDependency: c,
        pkgPath: n
      }
    };
    Object.assign(t, _);
  }
  getLocalDependencyRuntimeOnly(e) {
    var t, i, s, o;
    if (!e.isLocal()) return;
    const n = null !== (i = null === (t = e.getModuleJsonObj()) || undefined === t ? undefined : t.module.name) && undefined !== i ? i : e.getPackageName(),
      r = null !== (s = har_target_util_js_1.HarTargetUtil.calDepHarTargets(this.targetService).get(n)) && undefined !== s ? s : "default";
    return null === (o = build_mode_manager_js_1.buildOptionManager.getTargetBuildOption(n, r).arkOptions) || undefined === o ? undefined : o.runtimeOnly;
  }
  getLocalDependencyDynamicImportList(e) {
    return this.collectDynamicImport(this.getLocalDependencyRuntimeOnly(e));
  }
  getRemoteDependencyDynamicImportList(e) {
    var t;
    const i = e.getPackageFilePath(),
      s = json_util_js_1.JsonUtil.getJson5Obj(i);
    if (!s) return [];
    const o = null === (t = s.metadata) || undefined === t ? undefined : t.runtimeOnly;
    return this.collectDynamicImport(o);
  }
  processIndirectDependency(e) {
    e.forEach(e => {
      const t = e.isLocal() ? this.getLocalDependencyDynamicImportList(e) : this.getRemoteDependencyDynamicImportList(e);
      0 !== t.length && this.processIndirectDependencyInfo(e, t);
    });
  }
  collectDynamicImport(e) {
    var t, i;
    return [...(null !== (t = null == e ? undefined : e.sources) && undefined !== t ? t : []), ...(null !== (i = null == e ? undefined : e.packages) && undefined !== i ? i : [])];
  }
  processIndirectDependencyInfo(e, t) {
    t.forEach(t => {
      if (this.isHarDependency(t) && this.writeIndirectDependencyInfo(this.service.getHarDependencies(), t), this.isHspDependency(t) && this.writeIndirectDependencyInfo(this.service.getHspDependencies(), t), common_const_js_1.ValidateRegExp.SO_DEPENDENCY_REG_EXP.test(t) && this.processIndirectSoDependencyInfo(e, t), common_const_js_1.ValidateRegExp.RELATIVE_PATH_REG_EXP.test(t)) {
        const i = path_1.resolve(e.getDependencyRootPath(), t);
        if (!fse.existsSync(i)) return;
        const s = fse.statSync(i);
        s.isFile() && this.compileEntry.push(i), s.isDirectory() && this.processDirectoryImport(i);
      }
    });
  }
  processDirectoryImport(e) {
    file_util_js_1.FileUtil.getAllFilesFromFolder(e).forEach(e => {
      common_const_js_1.ValidateRegExp.DYNAMIC_IMPORT_FILE_SUFFIX_REG_EXP.test(file_util_js_1.FileUtil.getFileSuffix(e)) && fse.existsSync(e) && this.compileEntry.push(e);
    });
  }
  processIndirectSoDependencyInfo(e, t) {
    const i = e.getDependencyRootPath(),
      s = e.getDependencyMainFilePath(),
      o = e.getPackageFilePath(),
      n = dependency_manager_js_1.DependencyManager.isLocalDependencyForSo(s);
    if (!this.existInDynamicImportLibsInfo(t) && common_const_js_1.ValidateRegExp.SO_DEPENDENCY_REG_EXP.test(t)) {
      let s = n ? path_1.resolve(i, common_const_js_1.CommonConst.OH_MODULES, t, common_const_js_1.CommonConst.OH_PACKAGE_JSON5) : path_1.resolve(i.substring(0, i.lastIndexOf(path_1.sep)), t, common_const_js_1.CommonConst.OH_PACKAGE_JSON5);
      s = oh_package_loader_js_1.ohPackageLoader.getNodeOhPackagePath(s);
      const r = json_util_js_1.JsonUtil.getJson5Obj(s);
      if (!r) return;
      const a = path_1.resolve(i, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, build_directory_const_js_1.BuildDirConst.CPP_TYPES, t, r.types),
        l = {
          [t]: {
            hostModulesInfo: [{
              hostDependencyName: t,
              hostModuleName: this.getHostModuleName(e)
            }],
            moduleName: r.name,
            entryFilePath: a,
            isLocalDependency: n,
            pkgPath: o.substring(0, o.lastIndexOf(path_1.sep))
          }
        };
      Object.assign(this.dynamicImportLibInfo, l);
    }
  }
  writeIndirectDependencyInfo(e, t) {
    var i, s;
    const o = null === (s = null === (i = this.targetService.getBuildOption()) || undefined === i ? undefined : i.arkOptions) || undefined === s ? undefined : s.runtimeOnly,
      n = new Set((0, exclude_remote_dynamic_import_js_1.collectFromBuildOption)(null != o ? o : {}));
    e.forEach(e => {
      if (e.getDependencyName() === t) {
        const i = this.isHarDependency(t);
        i && !(0, exclude_remote_dynamic_import_js_1.isNeedToExcludeHar)(n, this.service.getRemoteHarDependencies(), e.getDependencyName()) && this.collectDepEtsFolderFilesEntry(e);
        const s = e.getDependencyMainFilePath(),
          o = fse.existsSync(s) ? s : e.getDependencyTypesFilePath();
        if (!fse.existsSync(o)) return;
        if (i && !e.isByteCodeHarDependency() && !(0, exclude_remote_dynamic_import_js_1.isNeedToExcludeHar)(n, this.service.getRemoteHarDependencies(), e.getDependencyName())) {
          const e = fse.statSync(o);
          e.isFile() ? this.compileEntry.push(o) : e.isDirectory() && file_util_js_1.FileUtil.getAllFilesFromFolder(o).forEach(e => {
            common_const_js_1.ValidateRegExp.DYNAMIC_IMPORT_FILE_SUFFIX_REG_EXP.test(file_util_js_1.FileUtil.getFileSuffix(e)) && this.compileEntry.push(e);
          });
        }
        if (this.existInDynamicImportLibsInfo(t)) return;
        const r = path_1.resolve(e.getDependencyRootPath(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN),
          a = path_1.resolve(r, common_const_js_1.CommonConst.MODULE_JSON),
          l = path_1.resolve(r, common_const_js_1.CommonConst.MODULE_JSON5),
          d = fse.existsSync(a) ? a : fse.existsSync(l) ? l : "",
          c = json_util_js_1.JsonUtil.getJson5Obj(d);
        if (!c) return;
        const h = c.module.type,
          u = h === module_type_enum_js_1.ModuleType.Har ? this.getHarDependencyModuleName(e, t) : h === module_type_enum_js_1.ModuleType.Shared ? c.module.name : "",
          p = e.getPackageFilePath(),
          _ = {
            [t]: {
              hostModulesInfo: [{
                hostDependencyName: t,
                hostModuleName: this.moduleName
              }],
              moduleName: u,
              entryFilePath: o,
              isLocalDependency: e.isLocal(),
              pkgPath: p.substring(0, p.lastIndexOf(path_1.sep))
            }
          };
        Object.assign(this.dynamicImportLibInfo, _);
      }
    });
  }
  getEntryMainFilePath(e, t) {
    var i, s;
    const o = path_1.resolve(e, (0, hvigor_arkts_compose_1.findNodeEntryFiles)(t)[0]);
    if (fse.existsSync(o)) return o;
    {
      const o = path_1.resolve(e, null !== (s = null !== (i = t.main) && undefined !== i ? i : t.types) && undefined !== s ? s : build_directory_const_js_1.BuildArtifactConst.INDEX_JS);
      return fse.existsSync(o) ? o : "";
    }
  }
  getHostModuleName(e) {
    var t;
    const i = e.getDependencyRootPath(),
      s = path_1.resolve(i, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, `${e.isLocal() ? common_const_js_1.CommonConst.MODULE_JSON5 : common_const_js_1.CommonConst.MODULE_JSON}`),
      o = json_util_js_1.JsonUtil.getJson5Obj(s);
    return o ? null === (t = o.module) || undefined === t ? undefined : t.name : "";
  }
  getHarDependencyModuleName(e, t) {
    var i;
    if (!this.isHarDependency(t)) return "";
    const s = e.getDependencyRootPath(),
      o = path_1.resolve(s, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, common_const_js_1.CommonConst.MODULE_JSON5),
      n = json_util_js_1.JsonUtil.getJson5Obj(o);
    return e.isLocal() ? null !== (i = null == n ? undefined : n.module.name) && undefined !== i ? i : "" : dependency_manager_js_1.DependencyManager.getModuleNameFromOhPkgJson(s);
  }
  existInDynamicImportLibsInfo(e) {
    return Object.prototype.hasOwnProperty.call(this.dynamicImportLibInfo, e);
  }
  isHarDependency(e) {
    const t = [];
    return this.service.getHarDependencies().forEach(e => {
      t.push(e.getDependencyName());
    }), t.includes(e);
  }
  isHspDependency(e) {
    const t = [];
    return this.service.getHspDependencies().forEach(e => {
      t.push(e.getDependencyName());
    }), t.includes(e);
  }
  processRouterMap() {
    var e;
    const t = this.pathInfo.getIntermediatesRouterMap(build_directory_const_js_1.BuildArtifactConst.LOADER_ROUTER_MAP_FILE_NAME);
    if (!fse.existsSync(t)) return;
    const i = json_util_js_1.JsonUtil.getJson5Obj(t);
    (null === (e = i.routerMap) || undefined === e ? undefined : e.length) && (i.routerMap.forEach(e => {
      this.compileEntry.push(e.pageSourceFile);
    }), Object.assign(this.intermediatesRouterMapObjs, i.routerMap));
  }
  processUpdateVersionInfo() {
    const e = this.targetService.getModuleService().getModuleModel().isHapModule() || this.targetService.getModuleService().getModuleModel().isHspModule(),
      t = this.targetName === common_const_js_1.CommonConst.OHOS_TEST,
      i = inject_util_js_1.InjectUtil.isLocalTest(),
      s = this.isPreview || t || i,
      o = this.targetService.getModuleService().getModuleModel().isHarModule();
    if (!(e || o && s)) return;
    (this.isByteCodeHarOptimize ? this.service.getAllByteCodeHarDependencies() : this.targetService.getModuleService().getHarDependencies().filter(e => e.isByteCodeHarDependency())).forEach(e => {
      var t, i, s;
      const o = e.getPackageJsonObj();
      (i = this.updateVersionInfo)[s = e.getPackageName()] || (i[s] = {});
      const n = null === (t = null == o ? undefined : o.metadata) || undefined === t ? undefined : t.dependencyPkgVersion;
      n ? Object.entries(n).filter(([e, t]) => {
        var i;
        const s = null === (i = this.pkgContextInfos[e]) || undefined === i ? undefined : i.version;
        return undefined !== s && "" !== s.trim() && s !== t;
      }).forEach(([t]) => {
        var i;
        this.updateVersionInfo[e.getPackageName()][t] = null === (i = this.pkgContextInfos[t]) || undefined === i ? undefined : i.version;
      }) : Object.entries({
        ...(null == o ? undefined : o.dependencies),
        ...(null == o ? undefined : o.dynamicDependencies)
      }).forEach(([t]) => {
        var i;
        const s = null === (i = this.pkgContextInfos[t]) || undefined === i ? undefined : i.version;
        s && "" !== s.trim() && (this.updateVersionInfo[e.getPackageName()][t] = s);
      });
    });
  }
  processBundledDependencies() {
    const e = har_target_util_js_1.HarTargetUtil.getDep2ConfigMap(this.targetService);
    this.targetService.isByteCodeHar() && this.isBundledDependencies && e && (this.pushConfigToCompileEntry(e.dep2Workers), this.pushDepRuntimeOnlyToCompileEntry(null == e ? undefined : e.dep2RuntimeOnlyObj), this.pushConfigToCompileEntry(e.dep2RouterMapObjPageSourceFiles));
  }
  addHarDepInsightSrcEntry() {
    if (this.moduleModel.isHarModule() && !this.isBundledDependencies) return;
    const e = new Map();
    har_target_util_js_1.HarTargetUtil.getHarPathToResourcesDirMap(e, this.targetService);
    this.service.getHarDependencies().forEach(t => {
      var i;
      null === (i = e.get(t.getDependencyRootPath())) || undefined === i || i.forEach(e => {
        var i;
        const s = t.getInsightIntentPathByResourceDir(e);
        if (t.isByteCodeHarDependency() || !fse.existsSync(s)) return;
        const o = json_util_js_1.JsonUtil.getJson5Obj(s),
          n = t.getDependencySrcMainPath();
        for (const e of [...(null !== (i = o.insightIntentsSrcEntry) && undefined !== i ? i : [])]) if (e.srcEntry) {
          const t = path_1.resolve(n, e.srcEntry);
          fse.existsSync(t) && this.compileEntry.push(t);
        }
      });
    });
  }
  addResourceTable2CompileEntry(e) {
    const t = this.sdkInfo.getSdkVersion() < 23;
    this.moduleModel.getModuleType() === module_type_enum_js_1.ModuleType.Har || inject_util_js_1.InjectUtil.isPreviewProcess() || t || Array.isArray(e.compileEntry) && e.compileEntry.push(this.pathInfo.getResourceTableTs());
  }
  collectDepAbilitySrcEntryArr() {
    const e = [];
    this.service.getHarDependencies().forEach(t => {
      var i;
      if (t.isByteCodeHarDependency()) return;
      const s = t.getModuleJsonObj();
      if (s) for (const o of [common_const_js_1.AbilityConst.UI_ABILITY, common_const_js_1.AbilityConst.EXTENSION_ABILITY]) {
        const n = null === (i = null == s ? undefined : s.module) || undefined === i ? undefined : i[o];
        (null == n ? undefined : n.length) && n.forEach(i => {
          if (!i.srcEntry) return;
          if (o === common_const_js_1.AbilityConst.EXTENSION_ABILITY && i.srcEntry.endsWith(".so")) return;
          const s = this.targetService.getAbsoluteSrcEntryPath(i.srcEntry, t.getDependencyRootPath());
          fse.existsSync(s) ? e.push(s) : this._log.debug("The srcEntry path for obtaining the ability of the module on which the module depends is incorrect or the corresponding file does not exist.");
        });
      }
    }), this.compileEntry.push(...e);
  }
  pushDepRuntimeOnlyToCompileEntry(e) {
    e && (this.pushConfigToCompileEntry(e.dep2RuntimeOnlySources), e.dep2RuntimeOnlyPackages.forEach(e => {
      e.forEach(e => {
        const t = this.service.getHarDependencies().find(t => t.getDependencyName() === e);
        if (!t) return;
        const i = hvigor_1.Json5Reader.getJson5Obj(t.getPackageFilePath()).main ? t.getDependencyMainFilePath() : t.getDependencyTypesFilePath();
        fse.existsSync(i) && this.compileEntry.push(i);
      });
    }));
  }
  pushConfigToCompileEntry(e) {
    (null == e ? undefined : e.size) && e.forEach((e, t) => {
      e.forEach(e => {
        const i = path_1.resolve(t.getDependencyRootPath(), e);
        fse.existsSync(i) && this.compileEntry.push(i);
      });
    });
  }
  shouldProcessOhExports() {
    return !!this.ohExportsFilePaths && (this.moduleModel.isHarModule() || this.moduleModel.isHspModule());
  }
  addOhExports2CompileEntry(e) {
    e.forEach(e => {
      this.compileEntry.push(e);
    });
  }
  writeOhExportsToLoaderJson(e, t) {
    e.length > 0 && (t[common_const_js_1.CommonConst.OH_EXPORTS] = this.ohExportsFilePaths);
  }
  checkOhExports() {
    const e = this.packageJsonObj[common_const_js_1.CommonConst.OH_EXPORTS];
    if (!e) return;
    this.useNormalizedOHMUrl || this._log.printErrorExit("OH_EXPORTS_NOT_SUPPORTED_WHEN_USENORMALIZEDOHMURL_IS_NOT_TRUE", [this.packageManagerPath]);
    const t = new Set(),
      i = new Set(),
      s = new Set(),
      o = new Set();
    e.forEach(e => {
      0 === e.length && t.add(e), path_1.isAbsolute(e) && t.add(e);
      const n = path_1.resolve(this.modulePath, e);
      if (fse.existsSync(n)) {
        n.startsWith(this.modulePath + path_1.sep) || t.add(e), i.has(n) ? s.add(e) : i.add(n);
        fse.statSync(n).isFile() && !/\.(ets|ts|js)$/.test(n) && o.add(e);
      } else t.add(e);
    }), t.size > 0 && this._log.printErrorExit("OH_EXPORTS_PATH_IS_INVALID", [[...t], this.packageManagerPath]), o.size > 0 && this._log.printErrorExit("OH_EXPORTS_NOT_SUPPORTED_FILE", [[...o], this.packageManagerPath]), s.size > 0 && this._log.warn(`Duplicate path: [${[...s]}] in 'oh-exports', at file: ${this.packageManagerPath}`);
  }
  processHarDeduplicate() {
    this.moduleModel.isHapModule() ? (this.addSourceHarDeduplicateFiles(), this.addDuplicateHarInfo()) : this.moduleModel.isHspModule() && this.processHspHarDeduplicate();
  }
  addSourceHarDeduplicateFiles() {
    this.allHspDepTargets && this.allHspDepTargets.forEach((e, t) => {
      const i = e.getPathInfo().getSourceHarDuplicateFilesInfoPath();
      if (!fse.existsSync(i)) return;
      const s = file_util_js_1.FileUtil.readLines(i);
      s && (this.compileEntry.push(...s), log.debug(`Deduplicate add compileEntry ${s}`));
    });
  }
  addDuplicateByteCodeHarInfoAndOtherCompileFiles() {
    if (!this.allHspDepTargets) return;
    const e = {},
      t = new Set();
    this.allHspDepTargets.forEach((i, s) => {
      const o = i.getPathInfo().getHarDuplicateInfoPath();
      if (!fse.existsSync(o)) return;
      const n = fse.readJSONSync(o),
        r = n.byteCodeHarInfo || {},
        a = n.otherCompileFiles || [];
      Object.keys(r).forEach(t => {
        this.byteCodeHarInfo[t] || e[t] || (e[t] = r[t]);
      }), a.forEach(e => {
        t.add(e);
      });
    }), log.debug(`Deduplicate needAddByteCodeHarInfo ${e}`), Object.assign(this.byteCodeHarInfo, e), t.forEach(e => {
      this.otherCompileFiles.includes(e) || this.otherCompileFiles.push(e);
    }), log.debug(`Deduplicate needAddOtherCompileFilesSet ${Array.from(t)}`);
  }
  addDuplicateHarInfo() {
    if (!this.allHspDepTargets) return;
    const e = new Set(),
      t = new Set();
    this.allHspDepTargets.forEach(i => {
      const s = i.getPathInfo().getHarDuplicateInfoPath();
      if (!fse.existsSync(s)) return;
      const o = fse.readJSONSync(s),
        n = o.byteCodeHarInfo || {},
        r = o.pkgContextInfo || {},
        a = o.updateVersionInfo || {},
        l = o.declarationEntry || [],
        d = o.compileEntry || [],
        c = o.workers || [],
        h = o.dynamicImportLibInfo || {};
      let u;
      const p = i.getPathInfo().getByteCodeHarDuplicateFilesInfoPath();
      fse.existsSync(p) && (u = file_util_js_1.FileUtil.readLines(p)), Object.keys(n).forEach(i => {
        this.updateVersionInfo[i] || (this.updateVersionInfo[i] = a[i], log.debug(`Deduplicate add updateVersion ${a[i]}`));
        const s = r[i];
        u && (0, deduplicate_util_js_1.processByteCodeHarDeclarationEntry)(i, u, s, t, e);
      }), (0, deduplicate_util_js_1.mergeDeclarationEntry)(l, this.declarationEntry), (0, deduplicate_util_js_1.mergeCompileEntry)(d, this.compileEntry), this.workerConfig && (0, deduplicate_util_js_1.mergeWorkers)(c, this.workerConfig), (0, deduplicate_util_js_1.mergeDynamicImportLibInfo)(h, this.dynamicImportLibInfo);
    }), (0, deduplicate_util_js_1.mergeDeclarationEntry)(e, this.declarationEntry);
  }
  collectHspDuplicateByteCodeHarInfoAndOtherCompileFiles() {
    const e = (0, collect_duplicate_har_dependencies_js_1.getHspNeedDeduplicateHarInfoMap)(this.moduleName);
    if (!e) return;
    const {
        byteCodeHarPkgNameSet: t,
        sourceHarWithoutSoPkgPathSet: i
      } = (0, deduplicate_util_js_1.getFilteredDuplicateHarInfo)(e),
      s = {},
      o = {};
    t.forEach(e => {
      if (this.byteCodeHarInfo[e] && (s[e] = this.byteCodeHarInfo[e], delete this.byteCodeHarInfo[e], log.debug(`Deduplicate byteCodeHar ${e} ${s[e]}`), this.pkgContextInfos[e])) {
        o[e] = this.pkgContextInfos[e];
        const t = (0, ohmurl_utils_js_1.generateOhmUrl)(this.pkgContextInfos[e]);
        Object.assign(this.harNameOhmMap, {
          [e]: t
        });
      }
    }), this.byteCodeHarInfoDuplicate = s, this.pkgContextInfoDuplicate = o, this.otherCompileFilesDuplicate = (0, deduplicate_util_js_1.deduplicateOtherCompileFiles)(this.otherCompileFiles, i);
  }
  processHspHarDeduplicate() {
    const e = (0, collect_duplicate_har_dependencies_js_1.getHspNeedDeduplicateHarInfoMap)(this.moduleName);
    if (!e) return;
    const {
        byteCodeHarPkgNameSet: t,
        sourceHarPkgPathSet: i,
        sourceHarWithoutSoPkgPathSet: s
      } = (0, deduplicate_util_js_1.getFilteredDuplicateHarInfo)(e),
      o = [],
      n = {};
    t.forEach(e => {
      this.updateVersionInfo[e] && (n[e] = this.updateVersionInfo[e], log.debug(`Deduplicate updateVersion ${e} ${n[e]}`)), o.push(...(0, deduplicate_util_js_1.deduplicateDeclarationEntry)(e, this.declarationEntry));
    }), fse.outputJSONSync(this.pathInfo.getHarDuplicateInfoPath(), {
      workers: this.workerConfig ? (0, deduplicate_util_js_1.deduplicateWorkers)(this.workerConfig, s) : [],
      byteCodeHarInfo: this.byteCodeHarInfoDuplicate,
      pkgContextInfo: this.pkgContextInfoDuplicate,
      compileEntry: (0, deduplicate_util_js_1.deduplicateCompileEntry)(this.compileEntry, s),
      declarationEntry: (0, wdk_1.uniq)(o),
      otherCompileFiles: this.otherCompileFilesDuplicate,
      updateVersionInfo: n,
      dynamicImportLibInfo: (0, deduplicate_util_js_1.deduplicateDynamicImportLibInfoObj)(this.dynamicImportLibInfo, i)
    });
  }
}
__decorate([(0, hvigor_1.Input)()], GenerateLoaderJson.prototype, "isByteCodeHarOptimize", null);
