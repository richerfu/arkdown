/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract/abstract-compile-node.js
 */
import os_1 from "os";
import * as path from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as const_1 from "@ohos/hvigor/src/common/const/const";
import fs_extra_1 from "fs-extra";
import * as ini_1 from "ini";
import * as find_target_product_js_1 from "../../common/find-target-product";
import * as project_ohos_config_manager_js_1 from "../../common/global/project-ohos-config-manager";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as aot_compile_mode_enum_js_1 from "../../enum/aot-compile-mode-enum";
import * as code_type_enum_js_1 from "../../enum/code-type-enum";
import * as compile_mode_enum_js_1 from "../../enum/compile-mode-enum";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as build_mode_manager_js_1 from "../../project/build-option/build-mode-manager";
import * as build_option_util_js_1 from "../../project/build-option/build-option-util";
import * as sdk_info_extension_js_1 from "../../sdk/sdk-info-extension";
import * as array_util_js_1 from "../../utils/array-util";
import * as file_util_js_1 from "../../utils/file-util";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as json_util_js_1 from "../../utils/json-util";
import * as oh_package_loader_js_1 from "../../utils/loader/file/oh-package-loader";
import * as res_model_loader_js_1 from "../../utils/loader/file/res-model-loader";
import * as ohos_test_util_js_1 from "../../utils/ohos-test-util";
import * as global_project_data_service_js_1 from "../service/global-project-data-service";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
var __decorate = this && this.__decorate || function (e, t, i, o) {
  var s,
    r = arguments.length,
    a = r < 3 ? t : null === o ? o = Object.getOwnPropertyDescriptor(t, i) : o;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) a = Reflect.decorate(e, t, i, o);else for (var n = e.length - 1; n >= 0; n--) (s = e[n]) && (a = (r < 3 ? s(a) : r > 3 ? s(t, i, a) : s(t, i)) || a);
  return r > 3 && a && Object.defineProperty(t, i, a), a;
};
export class AbstractCompileNode extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e, t, i) {
    var o;
    super(e, {
      ...i,
      name: `${AbstractCompileNode.getCompileNodeTaskName(t, i.name)}`
    }), this.TSIMPORTSENDABLE = "ark.tsImportSendable", this.commonOption = {
      path: path.dirname(process.execPath),
      watchMode: "false"
    }, this.externalApiPaths = [], this.buildOption = e.getBuildOption();
    const s = e.getTargetData().getTargetName();
    this.codeType = t, this.codeModel = this.moduleModel.getSourceSetByTargetName(s).getCodeMap().get(this.codeType), this.isArkModule = this.service.isArkModule(), this.sourcePath = null === (o = this.codeModel) || undefined === o ? undefined : o.getSrcPath(), this.targetName === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET && !this.isFaMode && this.sourcePath && (0, ohos_test_util_js_1.isTemplatesAfterSourceCodeMigration)(path.resolve(this.sourcePath, "..")) && (this.sourcePath = this.pathInfo.getOhosTestIntermediatesSrcEts()), this.aceSuperVisualPath = path.resolve(this.moduleModel.getSourceRootByTargetName(s), "supervisual"), this.aceModuleJsonPath = this.pathInfo.getIntermediatesArkModuleJsonPath(), fs_extra_1.existsSync(this.aceModuleJsonPath) || (this.aceModuleJsonPath = path.resolve(this.pathInfo.getIntermediatesProcessProfile())), this.resourceTable = path.resolve(this.pathInfo.getIntermediatesRes(), build_directory_const_js_1.BuildArtifactConst.RESOURCE_TABLE_TXT), this.rawFileResource = path.resolve(this.pathInfo.getIntermediatesRes(), build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES, build_directory_const_js_1.BuildDirConst.RAW_FILE), this.resProfileDir = this.pathInfo.getIntermediatesResProfilePath(), this.aceBuildJsonDir = this.pathInfo.getIntermediatesLoaderPath(), this.aceModuleBuild = path.resolve(this.pathInfo.getInterMediatesLoaderOutPath(), this.codeType);
    const r = this.moduleModel.getSourceRootByTargetName(common_const_js_1.DefaultTargetConst.DEFAULT_TARGET);
    this.mainCodePath = path.resolve(r, this.codeType.toString().toLowerCase()), this.anBuildOutputPath = path.resolve(this.pathInfo.getInterMediatesLoaderOutPath(), aot_compile_mode_enum_js_1.AotCompileModeEnum.AN), this.isCrossplatform = this.moduleModel.isCrossplatformModule(this.projectModel.getArkUIXConfigJsonObj());
  }
  static getCompileNodeTaskName(e, t) {
    return e === code_type_enum_js_1.CodeType.ETS ? `${t}ArkTS` : `${t}${e.toUpperCase()}`;
  }
  collectCustomTypes() {
    return this.targetService.getCustomTypes() || [];
  }
  get expandImportPath() {
    var e;
    return this.moduleModel.isHarModule() || null === (e = this.targetService.getBuildOption().arkOptions) || undefined === e ? undefined : e.expandImportPath;
  }
  get autoLazyFilter() {
    var e, t, i, o, s, r, a, n, l;
    const d = null === (e = this.targetService.getBuildOption().arkOptions) || undefined === e ? undefined : e.autoLazyFilter;
    if (undefined === d || undefined === (null == d ? undefined : d.include) || undefined === (null == d ? undefined : d.exclude)) return d;
    const u = this.targetService.getModuleService().getModuleModel(),
      c = u.getProfileOpt(),
      h = this.targetService.getTargetData().getTargetName(),
      _ = null === (o = null === (i = null === (t = u.getTargetOptions().filter(e => e.name === h)[0].config) || undefined === t ? undefined : t.buildOption) || undefined === i ? undefined : i.arkOptions) || undefined === o ? undefined : o.autoLazyFilter;
    if (undefined !== _) return _;
    const p = null === (s = this.getTargetBuildOption(u, h).arkOptions) || undefined === s ? undefined : s.autoLazyFilter;
    if (undefined !== p) return p;
    const g = null === (a = null === (r = c.buildOption) || undefined === r ? undefined : r.arkOptions) || undefined === a ? undefined : a.autoLazyFilter;
    if (undefined !== g) return g;
    const m = null === (l = null === (n = (0, find_target_product_js_1.findTargetProduct)(u.getParentProject()).buildOption) || undefined === n ? undefined : n.arkOptions) || undefined === l ? undefined : l.autoLazyFilter;
    return undefined !== m ? m : undefined;
  }
  getTargetBuildOption(e, t) {
    var i, o, s;
    const r = e.getProfileOpt(),
      a = build_option_util_js_1.BuildOptionUtil.getOrDefaultBuildModeBinderSet(null !== (i = r.buildModeBinder) && undefined !== i ? i : []),
      n = build_mode_manager_js_1.BuildModeManager.getBuildMode(),
      l = build_mode_manager_js_1.buildOptionManager.getBuildOptNameFromMapping(a, n, t);
    let d = null !== (o = r.buildOptionSet) && undefined !== o ? o : [];
    return d = build_option_util_js_1.BuildOptionUtil.arrayDeduplication(d), build_option_util_js_1.BuildOptionCombiner.processCopyFrom(d), null !== (s = (0, array_util_js_1.getElementFromArr)(d, l)) && undefined !== s ? s : {};
  }
  async beforeAlwaysAction() {
    var e, t, i, o, s, r, a;
    const n = this.expandImportPath;
    if (this.userProjectConfig = {
      autoLazyImport: null !== (t = null === (e = this.targetService.getBuildOption().arkOptions) || undefined === e ? undefined : e.autoLazyImport) && undefined !== t ? t : true === hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.HvigorConfigConst.OHOS_DEFAULTS_AUTOLAZYIMPORT),
      allowEmptyBundleName: true === hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.EMPTY_BUNDLENAME),
      reExportCheckMode: this.getReExportCheckMode(),
      executionMode: hvigor_1.coreParameter.startParams.optimizationStrategy,
      usePathPlaceholder: true === hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.OHOS_ROLLUP_CACHE_USE_PATH_PLACEHOLDER),
      writeRollupCache: false !== hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.OHOS_ARK_COMPILE_WRITE_ROLLUP_CACHE),
      isExpandImportEnabled: !!(null == n ? undefined : n.enable),
      expandImportExclude: (null === (i = null == n ? undefined : n.exclude) || undefined === i ? undefined : i.length) ? null == n ? undefined : n.exclude.join(",") : "",
      lazyImportExclude: (null === (o = this.autoLazyFilter) || undefined === o ? undefined : o.exclude) ? this.autoLazyFilter.exclude.join(",") : "",
      lazyImportInclude: (null === (s = this.autoLazyFilter) || undefined === s ? undefined : s.include) ? this.autoLazyFilter.include.join(",") : "",
      ignoreCrossplatformCheck: this.isCrossplatform && this.moduleModel.isIgnoreCrossPlatformCheckModule(this.projectModel.getArkUIXConfigJsonObj()),
      isHarDeduplicateEnabled: this.targetService.shouldDoDeduplicateTask(),
      skipOhModulesLint: true === (null === (r = this.targetService.getBuildOption().arkOptions) || undefined === r ? undefined : r.skipOhModulesLint),
      enableStrictCheckOHModule: this.getEnableStrictCheckOHModule(),
      disableSendableCheckRules: this.getDisableSendableCheckRules(),
      useDeclarationFileSignature: true === hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.OHOS_ETS_LINTER_USE_DECLARATION_FILE_SIGNATURE),
      customizedOptions: this.customizedOptions,
      integratedHsp: this.getIntegratedHsp()
    }, this.targetService.shouldDoDeduplicateTask()) {
      let e = null === (a = this.targetService.getBuildOption().resOptions) || undefined === a ? undefined : a.idDefinedFilePath;
      e && (e = path.isAbsolute(e) ? e : path.resolve(this.projectModel.getProjectDir(), e), fs_extra_1.existsSync(e) && (this.userProjectConfig.idDefinedJson5 = fs_extra_1.statSync(e).mtime));
    }
    this.targetService.isIntegratedHsp() && (this.userProjectConfig.integratedHspUseGetCurrentBundleName = inject_util_js_1.InjectUtil.isIntegratedHspUseGetCurrentBundleName()), this.customTypes = this.collectCustomTypes();
  }
  getIntegratedHsp() {
    var e;
    return !!this.moduleModel.isHspModule() && (null === (e = this.targetService.getBuildOption().arkOptions) || undefined === e ? undefined : e.integratedHsp);
  }
  getDisableSendableCheckRules() {
    var e, t;
    return null !== (t = null === (e = this.targetService.getBuildOption().strictMode) || undefined === e ? undefined : e.disableSendableCheckRules) && undefined !== t ? t : [];
  }
  getReExportCheckMode() {
    var e, t;
    return null !== (t = null === (e = this.targetService.getBuildOption().arkOptions) || undefined === e ? undefined : e.reExportCheckMode) && undefined !== t ? t : "noCheck";
  }
  getEnableStrictCheckOHModule() {
    var e, t;
    return null !== (t = null === (e = this.targetService.getBuildOption().strictMode) || undefined === e ? undefined : e.enableStrictCheckOHModule) && undefined !== t && t;
  }
  get isByteCodeHarOptimize() {
    return inject_util_js_1.InjectUtil.isByteCodeHarOptimize();
  }
  declareInputs() {
    var e, t, i, o, s, r, a, n, l, d, u, c, h, _, p, g, m, v, f;
    const P = new Map().set("debuggable", this.targetService.isDebug()).set("isArk", this.isArkModule).set("needCoverageInsert", inject_util_js_1.InjectUtil.isOhosTestCoverage()).set(this.TSIMPORTSENDABLE, true === hvigor_1.hvigorCore.getParameter().getPropertyInternal(this.TSIMPORTSENDABLE)).set("customTypes", null !== (e = this.customTypes) && undefined !== e ? e : "").set("isByteCodeHarOptimize", this.isByteCodeHarOptimize);
    for (const [e, i] of Object.entries(null !== (t = this.packageJsonObj.dependencies) && undefined !== t ? t : {})) P.set(e, `${e}: ${i}`);
    return this.moduleModel.getModuleType() !== module_type_enum_js_1.ModuleType.Har || (null === (i = this.buildOption) || undefined === i ? undefined : i.debuggable) || P.set("sourceMapDir", null !== (o = hvigor_1.coreParameter.properties[const_1.OHOS_ARK_COMPILE_SOURCE_MAP_DIR]) && undefined !== o ? o : ""), (null === (s = this.buildOption) || undefined === s ? undefined : s.debuggable) || P.set("branchElimination", null !== (n = null === (a = null === (r = this.targetService.getBuildOption()) || undefined === r ? undefined : r.arkOptions) || undefined === a ? undefined : a.branchElimination) && undefined !== n && n), P.set("caseSensitiveCheck", null !== (u = null === (d = null === (l = this.targetService.getBuildOption()) || undefined === l ? undefined : l.strictMode) || undefined === d ? undefined : d.caseSensitiveCheck) && undefined !== u && u), P.set(common_const_js_1.CommonConst.USE_NORMALIZED_OHM_URL, this.getUseNormalizedOHMUrl()), P.set(common_const_js_1.CommonConst.STRICT_CHECKER_ONLY, null !== (_ = null === (h = null === (c = this.targetService.getBuildOption()) || undefined === c ? undefined : c.strictMode) || undefined === h ? undefined : h.strictCheckerOnly) && undefined !== _ && _), P.set(build_directory_const_js_1.BuildDirConst.TRANSFORM_LIB, null !== (m = null === (g = null === (p = this.targetService.getBuildOption()) || undefined === p ? undefined : p.arkOptions) || undefined === g ? undefined : g.transformLib) && undefined !== m ? m : ""), P.set("compatibleSdkVersionStage", this.compatibleSdkVersionStage || ""), P.set("skipOhModulesLint", true === (null === (f = null === (v = this.targetService.getBuildOption()) || undefined === v ? undefined : v.arkOptions) || undefined === f ? undefined : f.skipOhModulesLint)), this.userProjectConfig && Object.entries(this.userProjectConfig).forEach(([e, t]) => {
      P.set(e, t);
    }), P;
  }
  declareOutputFiles() {
    var e;
    const t = new hvigor_1.FileSet().addEntry(this.aceModuleBuild, {
      isDirectory: true
    });
    if (this.targetService.getAnBuildMode() === aot_compile_mode_enum_js_1.AotCompileModeEnum.AOT_PARTIAL && t.addEntry(this.anBuildOutputPath, {
      isDirectory: true
    }), this.moduleModel.getModuleType() === module_type_enum_js_1.ModuleType.Har && !(null === (e = this.buildOption) || undefined === e ? undefined : e.debuggable)) if (hvigor_1.coreParameter.properties[const_1.OHOS_ARK_COMPILE_SOURCE_MAP_DIR]) {
      const e = this.getSourceMapDir(hvigor_1.coreParameter.properties[const_1.OHOS_ARK_COMPILE_SOURCE_MAP_DIR]);
      t.addEntry(path.resolve(e, build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP), {
        isDirectory: false
      });
    } else t.addEntry(path.resolve(this.pathInfo.getDefaultSourceMapPath(), build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP), {
      isDirectory: false
    });
    return t;
  }
  declareInputFiles() {
    const e = new hvigor_1.FileSet().addEntry(this.aceBuildJsonDir, {
      isDirectory: true
    }).addEntries(this.getAllExistDependentMainPath()).addEntries(this.getAllExistHarSrcPath(), {
      isDirectory: true
    });
    fs_extra_1.existsSync(this.rawFileResource) && e.addEntry(this.rawFileResource, {
      isDirectory: true
    });
    this.targetService.getResourceDirs().length > 0 && fs_extra_1.existsSync(this.resourceTable) && e.addEntry(this.resourceTable), !this.service.hasLiteDevice() && fs_extra_1.existsSync(this.aceModuleJsonPath) && e.addEntry(this.aceModuleJsonPath), this.resProfileDir && fs_extra_1.existsSync(this.resProfileDir) && e.addEntry(this.resProfileDir, {
      isDirectory: true
    }), this.aceSuperVisualPath && fs_extra_1.existsSync(this.aceSuperVisualPath) && e.addEntry(this.aceSuperVisualPath, {
      isDirectory: true
    }), this.sourcePath && fs_extra_1.existsSync(this.sourcePath) && e.addEntry(this.sourcePath, {
      isDirectory: true
    });
    const t = json_util_js_1.JsonUtil.getJson5Obj(this.packageManagerPath);
    if (t) {
      const i = path.join(this.module.getNodeDir(), t.main || "Index.ets");
      fs_extra_1.existsSync(i) && fs_extra_1.statSync(i).isFile() && e.addEntry(i);
    }
    if (this.targetService.getTargetData().getTargetName() === common_const_js_1.CommonConst.OHOS_TEST) {
      const t = path.resolve(this.module.getNodeDir(), build_directory_const_js_1.BuildDirConst.SRC, common_const_js_1.CommonConst.OHOS_TEST);
      fs_extra_1.existsSync(t) && e.addEntry(t, {
        isDirectory: true
      });
    }
    return fs_extra_1.existsSync(this.pathInfo.getIntermediatesPkgContextInfoPath()) && e.addEntry(this.pathInfo.getIntermediatesPkgContextInfoPath()), e;
  }
  getAllExistHarSrcPath() {
    return this.service.getDependencyRootPaths().map(e => path.resolve(e, "src", "main", this.codeType)).filter(e => fs_extra_1.existsSync(e));
  }
  getAllExistDependentMainPath() {
    return this.service.getDependencyMainPaths().filter(e => fs_extra_1.existsSync(e));
  }
  taskShouldDo() {
    return undefined !== this.sourcePath;
  }
  moveReleaseMap(e, t, i = this.codeType) {
    const o = path.resolve(e.getPathInfo().getInterMediatesLoaderOutPath(), i, build_directory_const_js_1.BuildArtifactConst.RELEASE_MAP);
    if (!fs_extra_1.existsSync(o)) return;
    const s = path.resolve(this.getTaskTempDir(e), i, build_directory_const_js_1.BuildArtifactConst.RELEASE_MAP);
    fs_extra_1.existsSync(s) && fs_extra_1.removeSync(s), fs_extra_1.moveSync(o, s), t.debug(`move ${o} to ${s}`);
  }
  async beforeTask() {
    if (inject_util_js_1.InjectUtil.isColdReload() || fs_extra_1.removeSync(this.aceModuleBuild), this.projectModel.isCrossplatformProject()) {
      const e = this.projectModel.getCompileApiMetaByProduct((0, find_target_product_js_1.findTargetProduct)(this.projectModel).name),
        t = new sdk_info_extension_js_1.SdkInfoExtension(e).getArkUIXInfo();
      await t.setup();
      const i = t.getArkUIXRootPath();
      this.externalApiPaths.push(path.resolve(i));
    }
  }
  getTaskTempDir(e, t = "") {
    const i = path.resolve(this.getArkCompileCachePath(e), `${this.targetService.getBuildMode().toLowerCase()}${t}`);
    return fs_extra_1.existsSync(i) || fs_extra_1.mkdirsSync(i), i;
  }
  getArkCompileCachePath(e) {
    const t = path.resolve(super.getTaskTempDir(e), `${this.targetCompileMode}`);
    return fs_extra_1.existsSync(t) || fs_extra_1.mkdirsSync(t), t;
  }
  validateModuleJson(e) {
    const t = this.pathInfo.getIntermediatesArkModuleJsonPath(),
      i = json_util_js_1.JsonUtil.getJson5Obj(t);
    this.validateModulePage(e, i), this.validateAbilitiesAndExtensionAbilitiesSrcEntry(e, i);
  }
  initCommonArkConfig() {
    var e, t, i, o, s;
    const r = (0, wdk_1.cloneDeep)(null === (e = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === e ? undefined : e.appOpt),
      a = this.getPkgContextInfo();
    let n = this.getResolveConflictMode(),
      l = new Map();
    if (n) {
      const e = this.service.getDependencyName2RootPath();
      false === e ? (n = false, l = new Map()) : l = e;
    }
    const d = fs_extra_1.realpathSync.native(this.sdkInfo.getEtsLoader());
    return {
      moduleType: this.moduleModel.getModuleType(),
      perf: hvigor_1.AnalyzeModeMap.get(hvigor_1.hvigorCore.getParameter().getStartParamsInternal().analyze),
      targetName: `.${this.targetName}`,
      packageManagerType: this.getPackageManagerType(),
      localPropertiesPath: path.resolve(this.projectModel.getProjectDir(), "local.properties"),
      isPreview: false,
      isOhosTest: "ohosTest" === this.targetName,
      isLocalTest: inject_util_js_1.InjectUtil.isLocalTest(),
      buildMode: this.targetService.getBuildMode(),
      hotReloadBuild: hvigor_1.hvigorCore.getParameter().getStartParamsInternal().hotReloadBuild,
      watchMode: "false",
      aceProfilePath: this.resProfileDir,
      etsLoaderPath: d,
      modulePath: this.module.getNodeDir(),
      testFrameworkPar: {
        testMode: inject_util_js_1.InjectUtil.isOhosTestCoverage() ? common_const_js_1.CommonConst.OHOS_TEST : undefined,
        coveragePathFilter: inject_util_js_1.InjectUtil.getCoveragePathFilter(),
        coverageMode: inject_util_js_1.InjectUtil.getCoverageMode()
      },
      needCoverageInsert: inject_util_js_1.InjectUtil.isOhosTestCoverage(),
      debugLine: inject_util_js_1.InjectUtil.isDebugLineEnable(),
      projectTopDir: path.resolve(this.projectModel.getProjectDir()),
      compileSdkVersion: this.compileApiVersion,
      compatibleSdkVersion: this.compatibleApiVersion,
      originCompatibleSdkVersion: this.originCompatibleSdkVersion,
      compatibleSdkVersionStage: this.compatibleSdkVersionStage,
      bundleName: this.getBundleName(r),
      etsLoaderVersion: this.sdkInfo.getEtsComponentVersion(),
      etsLoaderReleaseType: this.sdkInfo.getEtsComponentReleaseType(),
      aotCompileMode: inject_util_js_1.InjectUtil.isOhosTestCoverage() ? aot_compile_mode_enum_js_1.AotCompileModeEnum.AOT_NULL : this.targetService.getAnBuildMode(),
      apPath: this.targetService.getApAbsolutePath(),
      entryModuleName: this.moduleName,
      entryModuleVersion: this.packageJsonObj.version,
      entryPackageName: this.getEntryPackageName(),
      allModuleNameHash: global_project_data_service_js_1.GlobalProjectDataService.getInstance().getAllModuleNameHash(),
      externalApiPaths: this.getExternalApiPaths(),
      compilerTypes: this.customTypes,
      isCrossplatform: this.isCrossplatform,
      hvigorPluginFile: this.getHvigorPluginFile(),
      compilePluginPath: this.moduleModel.getCompilePluginPath(),
      buildGeneratedProfilePath: path.resolve(this.pathInfo.getGenerateBuildProfileDir(), this.targetName),
      bundleType: this.getBundleType(r),
      arkTSVersion: this.targetData.getProduct().arkTSVersion,
      apiVersion: this.sdkInfo.getSdkVersion(),
      needCompleteSourcesMap: "ohosTest" === this.targetName || inject_util_js_1.InjectUtil.isOhosTestCoverage() || inject_util_js_1.InjectUtil.isLocalTest() || this.moduleModel.isHarModule() && !this.targetService.isOpenSource(),
      isFaMode: this.isFaMode,
      buildDir: this.pathInfo.getModuleBuildPath(),
      deviceTypes: this.moduleModel.getDeviceTypes(),
      pkgContextInfo: a,
      ohPackagePathMap: oh_package_loader_js_1.ohPackageLoader.OhPackagePathMap,
      dependencyAliasMap: this.getDependencyAliasMap(a),
      permission: this.moduleModel.getPermission(),
      useGetCurrentBundleName: inject_util_js_1.InjectUtil.isIntegratedHspUseGetCurrentBundleName() && this.targetService.isIntegratedHsp(),
      projectArkOption: null === (t = this.targetService.getBuildOption()) || undefined === t ? undefined : t.arkOptions,
      sourceMapDir: hvigor_1.coreParameter.properties[const_1.OHOS_ARK_COMPILE_SOURCE_MAP_DIR] ? this.getSourceMapDir(hvigor_1.coreParameter.properties[const_1.OHOS_ARK_COMPILE_SOURCE_MAP_DIR]) : undefined,
      branchElimination: null !== (o = null === (i = this.targetService.getBuildOption().arkOptions) || undefined === i ? undefined : i.branchElimination) && undefined !== o && o,
      transformLib: this.normalizeTransformLib(),
      tsImportSendable: true === hvigor_1.hvigorCore.getParameter().getPropertyInternal(this.TSIMPORTSENDABLE),
      resolveConflictMode: n,
      depName2RootPath: l,
      depName2DepInfo: this.service.getDependencyName2DepInfo(),
      depName2OhExports: this.service.getDependencyName2OhExportsMap(),
      rootPathSet: Array.from(global_project_data_service_js_1.GlobalProjectDataService.getInstance().getRootPathSet()),
      useNativeResolver: null === (s = hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.USE_NATIVE_RESOLVER)) || undefined === s || s,
      shouldEmitJs: this.targetCompileMode === compile_mode_enum_js_1.CompileModeEnum.JS_BUNDLE || inject_util_js_1.InjectUtil.isOhosTestCoverage() || !(true === hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.NO_EMIT_JS) && this.checkSdkTscSyntaxKind(d)),
      useTscResolve: true === hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.USE_TSC_RESOLVE),
      singleFileEmit: this.targetService.isDebug() && inject_util_js_1.InjectUtil.isSingleFileEmitEnabled() && !inject_util_js_1.InjectUtil.isOhosTestCoverage(),
      userProjectConfig: this.userProjectConfig,
      executionMode: hvigor_1.coreParameter.startParams.optimizationStrategy,
      arkCompileCachePath: this.getArkCompileCachePath(this.targetData),
      autoLazyFilter: this.autoLazyFilter,
      isHarDeduplicateEnabled: this.targetService.shouldDoDeduplicateTask(),
      intermediateHarDuplicateDirPath: this.pathInfo.getIntermediatesHarDuplicatePath(),
      ...this.userProjectConfig,
      ...this.getStrictModeConfig()
    };
  }
  get customizedOptions() {
    var e;
    const t = null === (e = this.targetService.getBuildOption().packingOptions) || undefined === e ? undefined : e.customizedOptions;
    return (null == t ? undefined : t.basePackage) && (t.basePackage = path.resolve(this.module.getNodeDir(), t.basePackage)), t;
  }
  getStrictModeConfig() {
    var e, t, i, o, s, r, a;
    const n = null === (e = this.buildOption) || undefined === e ? undefined : e.strictMode;
    return (null == n ? undefined : n.useNormalizedOHMUrl) && undefined === (null == n ? undefined : n.noExternalImportByPath) && Object.assign(n, {
      noExternalImportByPath: true
    }), {
      strictMode: n,
      useNormalizedOHMUrl: this.getUseNormalizedOHMUrl(),
      caseSensitiveCheck: null !== (o = null === (i = null === (t = this.targetService.getBuildOption()) || undefined === t ? undefined : t.strictMode) || undefined === i ? undefined : i.caseSensitiveCheck) && undefined !== o && o,
      strictCheckerOnly: null !== (a = null === (r = null === (s = this.targetService.getBuildOption()) || undefined === s ? undefined : s.strictMode) || undefined === r ? undefined : r.strictCheckerOnly) && undefined !== a && a
    };
  }
  getPackageManagerType() {
    return this.isOhpmProject ? "ohpm" : "npm";
  }
  getBundleType(e) {
    var t, i;
    return null !== (i = null !== (t = null == e ? undefined : e.bundleType) && undefined !== t ? t : this.targetData.getProduct().bundleType) && undefined !== i ? i : this.projectModel.getBundleType();
  }
  getEntryPackageName() {
    return "ohosTest" === this.targetName && inject_util_js_1.InjectUtil.isOhosTestCoverage() ? `${this.packageJsonObj.name}_test` : this.packageJsonObj.name;
  }
  getBundleName(e) {
    var t, i;
    return null !== (i = null !== (t = null == e ? undefined : e.bundleName) && undefined !== t ? t : this.targetData.getProduct().bundleName) && undefined !== i ? i : this.projectModel.getDefaultBundleName();
  }
  checkSdkTscSyntaxKind(e) {
    try {
      const t = require(path.join(e, "node_modules", "typescript")).SyntaxKind;
      return !!t && !!(t.ImportDeclaration && t.ExportDeclaration && t.CallExpression && t.ImportEqualsDeclaration);
    } catch (e) {
      return false;
    }
  }
  getResolveConflictMode() {
    const e = e => {
      if (fs_extra_1.existsSync(e)) {
        return (0, ini_1.parse)(fs_extra_1.readFileSync(e, {
          encoding: "utf-8"
        })).resolve_conflict;
      }
    };
    let t = e(path.resolve(this.projectModel.getProjectDir(), ".ohpmrc"));
    return undefined !== t ? t : (t = e(path.resolve(os_1.homedir(), ".ohpm/.ohpmrc")), undefined === t || t);
  }
  normalizeTransformLib() {
    var e, t;
    if (!inject_util_js_1.InjectUtil.isInTest() && this.targetService.isSourceCodeHar()) return;
    const i = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.transformLib;
    return i ? path.resolve(this.pathInfo.getModulePath(), i) : undefined;
  }
  getBlockDependencies() {
    return this.isByteCodeHarOptimize ? this.service.getRootHspDependencies().concat(this.service.getRootByteCodeHarDependencies()) : this.service.getHspOrByteCodeHarDependencies();
  }
  async initDefaultArkCompileConfig() {
    const e = this.moduleModel.isHspModule() || this.targetService.isByteCodeHar(),
      t = this.pathInfo.getIntermediatesEtsTgzPath(this.moduleName);
    let i, o;
    return this.moduleModel.isHarModule() || inject_util_js_1.InjectUtil.isPreviewProcess() || (i = this.pathInfo.getResourceTableTs(), o = this.getModuleMetaInfoFiles()), {
      ...this.initCommonArkConfig(),
      aceModuleJsonPath: this.aceModuleJsonPath,
      appResource: this.resourceTable,
      rawFileResource: this.rawFileResource,
      resourceTableHash: (0, hvigor_1.hashFile)(this.resourceTable),
      runtimeOS: this.sdkInfo.isOhos ? common_const_js_1.CommonConst.OPEN_HARMONY : common_const_js_1.CommonConst.HARMONY_OS,
      sdkInfo: `${this.sdkInfo.isOhos}:${this.sdkInfo.getSdkVersion()}:${this.sdkInfo.getEtsComponentVersion()}:${this.sdkInfo.getEtsComponentReleaseType()}`,
      aceModuleRoot: this.sourcePath,
      compileMode: this.targetCompileMode,
      aceSuperVisualPath: this.aceSuperVisualPath,
      aceBuildJson: path.resolve(this.aceBuildJsonDir, build_directory_const_js_1.BuildArtifactConst.LOADER_JSON),
      cachePath: this.getTaskTempDir(this.targetData),
      aceModuleBuild: this.aceModuleBuild,
      supportChunks: true,
      declaredFilesPath: e ? t : undefined,
      shouldCollectDirectImportees: this.isByteCodeHarOptimize,
      isByteCodeHarOptimize: this.isByteCodeHarOptimize,
      expandImportPath: this.expandImportPath,
      resourceTableTs: i,
      moduleMetaInfoFiles: o
    };
  }
  async getPreviewCompileConfig(e) {
    const t = this.pathInfo.getBuildConfigPath(true);
    let i = {};
    fs_extra_1.existsSync(t) ? i = fs_extra_1.readJsonSync(t) : e.debug(`Build config file not found: ${t}`), i.cachePath && delete i.cachePath;
    const o = {
      ...(await this.initDefaultArkCompileConfig()),
      ...i
    };
    return o.obfuscationOptions && delete o.obfuscationOptions, o.watchMode = "true", o.isPreview = true, o.projectPath = o.aceModuleRoot, o.level = e.getLevel(), o.buildDir = this.pathInfo.getModulePreviewPath(), o.sdkPath = this.sdkInfo.sdkDir, o.uiTransformOptimization = false, fs_extra_1.ensureDirSync(o.cachePath), fs_extra_1.ensureDirSync(o.aceModuleBuild), o;
  }
  handleCompileEvents(e, t, i) {
    t && e && (e.forEach(e => {
      const i = Object.assign(e.head.type === hvigor_1.MetricEventType.DURATION ? new hvigor_1.DurationEvent("", "", "", 0, "", "") : new hvigor_1.ContinualEvent("", "", "", 0, ""), e);
      hvigor_1.MetricService.getInstance().submit(i), i.getParent() || (t.addChild(i.getId()), i.setParent(t.getId())), i.setLog(i.getName(), hvigor_1.MetricLogType.INFO, i.getDescription());
    }), i && this.updateTid(t));
  }
  validateAbility(e) {
    const t = this.targetData.getTargetName(),
      i = this.moduleModel.getSourceSetByTargetName(t).getLegacyModuleTargetRes().getJsonPath();
    if (!fs_extra_1.existsSync(i)) return;
    const o = res_model_loader_js_1.resModelLoader.getConfigJson(i),
      s = o.module.abilities ? o.module.abilities : [],
      r = path.resolve(this.moduleModel.getModule().getNodeDir(), build_directory_const_js_1.BuildDirConst.SRC, t === common_const_js_1.CommonConst.OHOS_TEST ? common_const_js_1.CommonConst.OHOS_TEST : build_directory_const_js_1.BuildDirConst.MAIN),
      a = new Map();
    s.forEach(t => {
      t.srcLanguage || e.printErrorExit("SRC_LANGUAGE_UNDEFINED", [t.name, o]);
      const s = t.srcLanguage ? t.srcLanguage : "",
        n = path.resolve(r, s, t.srcPath);
      fs_extra_1.existsSync(n) || e.printErrorExit("SRC_PATH_NOT_FOUND", [t.srcPath, n, i], [[t.srcPath]]), a.set(t.name, s);
    });
    (o.module.js ? o.module.js : []).forEach(t => {
      var o;
      if ("form" === t.type) return;
      const s = null !== (o = a.get(t.name)) && undefined !== o ? o : "";
      t.pages.forEach(o => {
        var a, n;
        const l = path.resolve(r, s, `${t.name.substring(1)}`, `${o}.${null !== (n = null === (a = t.mode) || undefined === a ? undefined : a.syntax) && undefined !== n ? n : "hml"}`);
        fs_extra_1.existsSync(l) || e.printErrorExit("MODULE_JS_PAGE_NOT_FOUND", [o, l, i], [[o]]);
      });
    });
  }
  updateTid(e) {
    e.getChildren().forEach(t => {
      const i = hvigor_1.MetricService.getInstance().getEventById(t);
      i && this.updateTid(i.setTid(e.getTid()));
    });
  }
  validateModulePage(e, t) {
    const i = t.module.pages;
    if (undefined === i) return;
    const o = `${i.replace(/\$profile:/, "")}.json`,
      s = path.resolve(this.resProfileDir, o);
    fs_extra_1.existsSync(s) || e.printErrorExit("MODULE_PAGE_NOT_FOUND", [i, s], [[o]]);
  }
  validateAbilitiesAndExtensionAbilitiesSrcEntry(e, t) {
    this.validateSrcEntry(e, t.module.abilities, common_const_js_1.AbilityConst.UI_ABILITY), this.validateSrcEntry(e, t.module.extensionAbilities, common_const_js_1.AbilityConst.EXTENSION_ABILITY);
  }
  validateSrcEntry(e, t, i) {
    const o = this.moduleModel.getSourceSetByTargetName(this.targetData.getTargetName());
    if (t && (!t || 0 !== t.length)) for (const s of t) {
      const t = s.srcEntry;
      if (undefined === t) continue;
      const r = path.isAbsolute(t) ? t : path.resolve(o.getSourceSetRoot(), t);
      if (this.targetName === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET && (0, ohos_test_util_js_1.isTemplatesAfterSourceCodeMigration)(o.getSourceSetRoot())) return;
      const a = i === common_const_js_1.AbilityConst.EXTENSION_ABILITY,
        n = t.endsWith(".so");
      if (a && n) return;
      if (this.targetService.isCustomizedHar()) return;
      if (!file_util_js_1.FileUtil.fileExists(r)) {
        const i = o;
        e.printErrorExit("MODULE_SRC_ENTRY_NOT_FOUND", [t, i.getModuleTargetRes().getJsonPath()]);
      }
    }
  }
  getHvigorPluginFile() {
    var e, t;
    let i = null === (t = null === (e = this.buildOption) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.compilePluginFile;
    if (i) {
      const e = path.resolve(this.moduleModel.getProjectDir(), i);
      i = fs_extra_1.existsSync(e) ? e : `${e}.ts`;
    }
    return i;
  }
  getExternalApiPaths() {
    return this.compileApiVersion > sdk_const_js_1.ApiVersion.API_VERSION_9 && this.targetData.isHarmonyOS() && this.sdkInfo.getHmsArkDir() && this.externalApiPaths.push(this.sdkInfo.getHmsArkDir()), this.externalApiPaths;
  }
  getUseNormalizedOHMUrl() {
    var e, t;
    return !!(null === (t = null === (e = this.buildOption) || undefined === e ? undefined : e.strictMode) || undefined === t ? undefined : t.useNormalizedOHMUrl);
  }
  getPkgContextInfo() {
    const e = this.pathInfo.getIntermediatesPkgContextInfoPath();
    return json_util_js_1.JsonUtil.getJson5Obj(e);
  }
  getDependencyAliasMap(e) {
    const t = new Map();
    return e ? (Object.values(e).forEach(e => {
      e.dependencyAlias && t.set(e.dependencyAlias, e.packageName);
    }), t) : t;
  }
  getSourceMapDir(e) {
    return path.isAbsolute(e) ? path.resolve(hvigor_1.coreParameter.properties[const_1.OHOS_ARK_COMPILE_SOURCE_MAP_DIR], this.moduleName) : path.resolve(this.projectModel.getProjectDir(), "hvigor", hvigor_1.coreParameter.properties[const_1.OHOS_ARK_COMPILE_SOURCE_MAP_DIR], this.moduleName);
  }
  getModuleMetaInfoFiles() {
    var e;
    const t = null !== (e = this.targetService.getTargetData().getProduct().bundleName) && undefined !== e ? e : this.projectModel.getDefaultBundleName(),
      i = this.getUseNormalizedOHMUrl(),
      o = [];
    return o.push({
      filePath: this.pathInfo.getResourceTableTs(),
      ohmurl: i ? `&${this.moduleName}/build/generated/r/ResourceTable&` : `${t}/${this.moduleName}/build/generated/r/ResourceTable`
    }), o;
  }
}
__decorate([(0, hvigor_1.Input)()], AbstractCompileNode.prototype, "autoLazyFilter", null), __decorate([(0, hvigor_1.Input)()], AbstractCompileNode.prototype, "customizedOptions", null);
