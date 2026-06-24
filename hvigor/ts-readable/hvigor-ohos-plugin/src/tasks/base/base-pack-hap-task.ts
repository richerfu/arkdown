/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/base/base-pack-hap-task.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import * as const_1 from "@ohos/hvigor/src/common/const/const";
import * as hvigor_config_loader_1 from "@ohos/hvigor/src/common/util/hvigor-config-loader";
import * as fse from "fs-extra";
import * as packing_tool_options_js_1 from "../../builder/inner-java-command-builder/packing-tool-options";
import * as project_ohos_config_manager_js_1 from "../../common/global/project-ohos-config-manager";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as aot_compile_mode_enum_js_1 from "../../enum/aot-compile-mode-enum";
import * as code_type_enum_js_1 from "../../enum/code-type-enum";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as dependency_interface_js_1 from "../../project/dependency/core/dependency-interface";
import * as dependency_manager_js_1 from "../../project/dependency/dependency-manager";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as integrated_hsp_utils_js_1 from "../../utils/integrated-hsp-utils";
import * as json_util_js_1 from "../../utils/json-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as process_utils_js_1 from "../../utils/process-utils";
import * as sourcemap_util_js_1 from "../../utils/sourcemap-util";
import * as task_util_js_1 from "../../utils/task-util";
import * as abstract_merge_profile_js_1 from "../abstract-merge-profile";
import * as cache_native_libs_js_1 from "../cache-native-libs";
import * as task_names_js_1 from "../common/task-names";
import * as generate_metadata_js_1 from "../generate-metadata";
import * as make_pack_info_js_1 from "../make-pack-info";
import * as process_module_privacy_profile_js_1 from "../pac-process/process-module-privacy-profile";
import * as process_integrated_hsp_js_1 from "../process-integrated-hsp";
import * as process_router_map_js_1 from "../process-router-map";
import * as process_share_config_js_1 from "../process-share-config";
import * as process_startup_config_js_1 from "../process-startup-config";
import * as module_task_service_js_1 from "../service/module-task-service";
import * as syscap_transform_js_1 from "../syscap-transform";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
const ohosTestTargetName = common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET;
export class BasePackHapTask extends ohos_hap_task_js_1.OhosHapTask {
  declareExecutionTool() {
    return this.sdkInfo.getPackageTool();
  }
  declareExecutionCommand() {
    const e = [];
    for (let t = 0; t < this.allPackInfoPath.length; t++) e.push(this.generateCommand(this.allPackInfoPath[t], this.allOutPath[t]).toString());
    return e.join();
  }
  declareInputs() {
    var e, t, s;
    const o = new Map().set("hotReload", inject_util_js_1.InjectUtil.isHotReload()).set(build_directory_const_js_1.BuildDirConst.INTEGRATED_HSP, this.integratedHsp).set(abstract_merge_profile_js_1.INPUT_KEY.projectConfigAppOpt, JSON.stringify(null === (e = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === e ? undefined : e.signingConfig));
    return this.moduleModel.getModuleType() !== module_type_enum_js_1.ModuleType.Har && (null === (t = this.buildOption) || undefined === t ? undefined : t.debuggable) && o.set("sourceMapDir", null !== (s = hvigor_1.coreParameter.properties[const_1.OHOS_ARK_COMPILE_SOURCE_MAP_DIR]) && undefined !== s ? s : ""), o.set("compressNativeLibs", JSON.stringify(this.moduleModel.getCompressNativeLib())), o;
  }
  declareInputFiles() {
    const e = new hvigor_1.FileSet().addEntries([this.libPath, this.jsonPath, this.resourcePath, this.indexPath, ...this.allPackInfoPath], {
      isDirectory: false
    });
    fse.existsSync(this.rpcidSc) && e.addEntry(this.rpcidSc, {
      isDirectory: false
    }), fse.existsSync(this.jsAssetsPath) && e.addEntry(this.jsAssetsPath, {
      isDirectory: false
    }), fse.existsSync(this.etsAssetsPath) && e.addEntry(this.etsAssetsPath, {
      isDirectory: false
    }), fse.existsSync(this.nodeModulesPath) && e.addEntry(this.nodeModulesPath, {
      isDirectory: true
    }), fse.existsSync(this.pkgContextInfoPath) && this.useNormalizedOHMUrl && e.addEntry(this.pkgContextInfoPath, {
      isDirectory: false
    }), fse.existsSync(this.pkgSdkInfoPath) && !this.useNormalizedOHMUrl && e.addEntry(this.pkgSdkInfoPath, {
      isDirectory: false
    });
    const t = this.getSourceMapSourcePath();
    fse.existsSync(t) && e.addEntry(t, {
      isDirectory: false
    });
    const s = this.targetService.getTargetData().getTargetName(),
      o = this.pathInfo.getGenerateBuildProfilePath(s);
    return fse.existsSync(o) && e.addEntry(o), e;
  }
  declareOutputFiles() {
    const e = new hvigor_1.FileSet().addEntries([...this.allOutPath], {
      isDirectory: false
    });
    if (this.integratedHsp && this.allOutPath.forEach(t => {
      e.addEntry(path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), path_1.basename(t)), {
        isDirectory: false
      });
    }), this.sdkInfo.hasRollUpPluginInEtsLoader) if (e.addEntry(path_1.resolve(this.sourceMapInterDirPath, build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP)), hvigor_1.coreParameter.properties[const_1.OHOS_ARK_COMPILE_SOURCE_MAP_DIR]) {
      const t = this.getSourceMapPath(hvigor_1.coreParameter.properties[const_1.OHOS_ARK_COMPILE_SOURCE_MAP_DIR]);
      e.addEntry(path_1.resolve(t, this.moduleName, build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP), {
        isDirectory: false
      });
    } else e.addEntry(path_1.resolve(this.pathInfo.getDefaultSourceMapPath(), build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP));
    return e;
  }
  constructor(e, t, s, o) {
    var i, a, r, n;
    super(e, t), this._log = ohos_logger_js_1.OhosLogger.getLoggerWithDurationId("base-pack-hap-task", this.durationEvent.getId()), this.allPackInfoPath = [], this.allOutPath = [], this.mode = s;
    const c = this.targetService.getTargetData(),
      d = c.getPathInfo();
    this.libPath = d.getIntermediatesStrippedLibsDir(), this.jsonPath = path_1.resolve(d.getIntermediatesRes(), common_const_js_1.CommonConst.MODULE_JSON), this.packageHapJsonPath = path_1.resolve(this.pathInfo.getPackageHapProductPath(), common_const_js_1.CommonConst.MODULE_JSON), this.pkgContextInfoPath = path_1.resolve(this.pathInfo.getIntermediatesLoaderPath(), common_const_js_1.CommonConst.PKG_CONTEXT_INFO_JSON), this.pkgSdkInfoPath = path_1.resolve(this.pathInfo.getIntermediatesLoaderPath(), common_const_js_1.CommonConst.PKG_SDK_INFO_JSON), this.resourcePath = path_1.resolve(d.getIntermediatesRes(), build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES), this.indexPath = path_1.resolve(d.getIntermediatesRes(), build_directory_const_js_1.BuildArtifactConst.RESOURCE_INDEX), this.rpcidSc = path_1.resolve(d.getIntermediatesSysCap(), common_const_js_1.CommonConst.RPCID_SC), this.jsAssetsPath = path_1.resolve(d.getInterMediatesLoaderOutPath(), code_type_enum_js_1.CodeType.JS), this.etsAssetsPath = path_1.resolve(d.getInterMediatesLoaderOutPath(), code_type_enum_js_1.CodeType.ETS), this.nodeModulesPath = path_1.resolve(d.getInterMediatesLoaderOutPath(), common_const_js_1.CommonNodeConst.NODE_MODULES), this.syscapJsonPath = path_1.resolve(null === (i = this.moduleModel) || undefined === i ? undefined : i.getSourceRootByTargetName(this.targetName), common_const_js_1.CommonConst.SYSCAP_JSON), this.sourceMapInterDirPath = d.getInterMediatesSourceMapDirPath(), this.fastAppDirPath = d.getFastAppStorageDirectory(), this.useNormalizedOHMUrl = null !== (n = null === (r = null === (a = this.targetService.getBuildOption()) || undefined === a ? undefined : a.strictMode) || undefined === r ? undefined : r.useNormalizedOHMUrl) && undefined !== n && n, this.anBuildOutputPath = path_1.resolve(this.pathInfo.getInterMediatesLoaderOutPath(), aot_compile_mode_enum_js_1.AotCompileModeEnum.AN), this.moduleBuildOpt = this.moduleModel.getProfileOpt(), this.buildOption = this.targetService.getBuildOption(), this.apDirPath = this.pathInfo.getIntermediatesApPath(), this.integratedHsp = this.targetService.isIntegratedHsp();
    const l = this.integratedHsp ? d.getIntegratedHspOutputPath() : d.getModuleBuildOutputPath();
    fse.existsSync(l) || fse.mkdirSync(l, {
      recursive: true
    });
    const _ = this.service.getRelatedEntryModules(),
      h = this.service.getModuleModel().getModuleType();
    if (module_task_service_js_1.ModuleTaskService.checkEntryModules(h, _)) for (const e of _) this.module.findModuleByName(e) && (this.allPackInfoPath.push(path_1.resolve(d.getModuleBuildOutputPath(), e, build_directory_const_js_1.BuildArtifactConst.PACK_INFO)), this.allOutPath.push(path_1.resolve(l, c.getModuleTargetOutputFileName(e, false))));else this.allPackInfoPath.push(path_1.resolve(d.getModuleBuildOutputPath(), build_directory_const_js_1.BuildArtifactConst.PACK_INFO)), this.allOutPath.push(path_1.resolve(l, c.getModuleTargetOutputFileName("", false, this.getSuffix())));
  }
  get alignDeviceTypes() {
    return this.projectModel.isFaMode() ? [] : inject_util_js_1.InjectUtil.getAlignDeviceTypes();
  }
  async doTaskAction() {
    for (let e = 0; e < this.allPackInfoPath.length; e++) await this.executeCommandList(this.allPackInfoPath[e], this.allOutPath[e]);
    this.generateInterSourceMap(), this.moveSourceMap(), await this.replaceSourceMapInCoverageMode();
  }
  async executeCommandList(e, t) {
    const s = this.generatePackInputData(e, t);
    await this.executePackInWorkerPool(s);
  }
  generatePackInputData(e, t) {
    const s = this.durationEvent.createSubEvent("generate HAP packaging command", "");
    s.start(), this.checkCompressNativeLib();
    const o = this.generateCommand(e, t);
    this.processIncrementalCommandForLib(t, o), this._log._printDebugCommand(this.sdkInfo.getPackageTool(), o);
    const i = {
      moduleName: this.moduleName,
      taskName: this.name,
      commandLine: o,
      integratedHspCommand: this.integratedHsp ? (0, integrated_hsp_utils_js_1.generateIntegratedHspCommand)(this.projectModel, this.sdkInfo.getPackageTool(), t, this.pathInfo.getModuleBuildOutputPath()) : undefined
    };
    return s.stop(), s.setLog(s.getName(), hvigor_1.MetricLogType.INFO), i;
  }
  async executePackInWorkerPool(e) {
    const t = "submit HAP packaging task to work pool",
      s = this.durationEvent.createSubEvent(t, "Pack HAP in work pool");
    s.start(), await new process_utils_js_1.ProcessUtils(e.moduleName, e.taskName).submitExecutionWithoutReturn(this, this.getWorkerPool(), e, wdk_1.noop, [], s), s.stop(), s.setLog(t, hvigor_1.MetricLogType.INFO);
  }
  checkCompressNativeLib() {
    this.moduleModel;
    const e = hvigor_config_loader_1.HvigorConfigLoader.getInstance().getPropertiesConfigValue(common_const_js_1.HvigorConfigConst.OHOS_PACK_COMPRESS_LEVEL);
    if (e && fse.existsSync(path_1.resolve(this.pathInfo.getModulePath(), "libs"))) {
      const t = common_const_js_1.HvigorConfigConst.OHOS_PACK_COMPRESS_LEVEL_MAP.get(e);
      if (this.compileApiVersion < 12) return void this._log.warn(`Invalid ohos.pack.compressLevel configuration, ohos.pack.compressLevel configuration \n      minimum requirement is api 12 version of the project, your project is api ${this.compileApiVersion} project`);
      t || this._log.warn(`invalid ohos.pack.compress parameter:${e}\n      ,It won't work, the packing will follow the fast compression level.`);
    }
  }
  generateCommand(e, t) {
    const s = new packing_tool_options_js_1.PackingToolOptions();
    s.addCalledJarFile(this.sdkInfo.getPackageTool()), s.addMode(this.mode).force(true).addLibPath(this.libPath).addJsonPath(this.packageHapJsonPath).addResourcesPath(this.priorResourcePath || this.resourcePath).addIndexPath(this.priorResourcesIndexPath || this.indexPath).addPackInfoPath(e).addOutPath(t);
    const o = hvigor_config_loader_1.HvigorConfigLoader.getInstance().getPropertiesConfigValue(common_const_js_1.HvigorConfigConst.OHOS_PACK_COMPRESS_LEVEL);
    return o && fse.existsSync(path_1.resolve(this.pathInfo.getModulePath(), "libs")) && s.addCompressLevel(o, this.sdkInfo.getSdkVersion()), this.targetService.getAnBuildMode() && (fse.existsSync(this.anBuildOutputPath) && s.addAnPath(this.anBuildOutputPath), fse.existsSync(this.apDirPath) && s.addDirList([this.apDirPath])), fse.existsSync(this.rpcidSc) && s.addSysCapPath(this.rpcidSc), fse.existsSync(this.jsAssetsPath) && s.addJsPath(this.jsAssetsPath), fse.existsSync(this.etsAssetsPath) && s.addEtsPath(this.etsAssetsPath), fse.existsSync(this.nodeModulesPath) && s.addDirList([this.nodeModulesPath]), fse.existsSync(this.pkgContextInfoPath) && this.useNormalizedOHMUrl && s.addPkgContextInfoPath(this.pkgContextInfoPath), fse.existsSync(this.pkgSdkInfoPath) && !this.useNormalizedOHMUrl && s.addPkgSdkInfoPath(this.pkgSdkInfoPath), s.build();
  }
  lastBuildPackCanBeReused() {
    var e;
    const t = null === (e = hvigor_1.ProjectCacheService.getInstance(this.projectModel.getProject())) || undefined === e ? undefined : e.getBuildInfo();
    this._log.debug("上次构建任务:", null == t ? undefined : t.get("_")), this._log.debug("上次构建 hotReloadBuild:", null == t ? undefined : t.get("hotReloadBuild")), this._log.debug("上次构建 hotCompile:", null == t ? undefined : t.get("hotCompile"));
    const s = Array.isArray(null == t ? undefined : t.get("_")) ? Array.from(null == t ? undefined : t.get("_")) : [];
    return !(null == s ? undefined : s.includes("assembleDevHqf")) && true !== (null == t ? undefined : t.get("hotReloadBuild")) && true !== (null == t ? undefined : t.get("hotCompile"));
  }
  processIncrementalCommandForLib(e, t) {
    var s;
    if (!(null === (s = this.targetService.getBuildOption().packOptions) || undefined === s ? undefined : s.enableIncrementalSoCompress)) return;
    if (!this.lastBuildPackCanBeReused()) return;
    const o = new hvigor_1.TaskProxy(this),
      i = o.getFileCompareResult(this.libPath)[1],
      a = o.taskInputValueChanged("compressNativeLibs");
    i || a || !fse.existsSync(e) ? this._log.debug("Libs under the folder have been changed, will not do incremental pack: ", this.libPath) : (this._log.info("Add incremental package command for libs."), this.addIncrementalHapPath(t, e), this.addIncrementalLibPackCommand(t));
  }
  addIncrementalHapPath(e, t) {
    e.push("--exist-src-path"), e.push(t);
  }
  addIncrementalLibPackCommand(e, t = "true") {
    e.push("--lib-path-retain"), e.push(t);
  }
  getSuffix() {
    switch (this.mode) {
      case "hap":
      default:
        return build_directory_const_js_1.BuildArtifactExtension.DOT_HAP;
      case "hsp":
        return build_directory_const_js_1.BuildArtifactExtension.DOT_HSP;
    }
  }
  getSourceMapSourcePath() {
    return this.targetService.isDebug() ? path_1.resolve(this.etsAssetsPath, build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP) : path_1.resolve(this.pathInfo.getModuleBuildCachePath(), `${this.targetName}@CompileArkTS`, "esmodule", "release", build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP);
  }
  generateInterSourceMap() {
    if (!this.sdkInfo.hasRollUpPluginInEtsLoader) return;
    const e = this.getSourceMapSourcePath();
    fse.existsSync(e) && (fse.ensureDirSync(this.sourceMapInterDirPath), fse.copyFileSync(e, path_1.resolve(this.sourceMapInterDirPath, build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP)));
  }
  moveSourceMap() {
    if (!this.sdkInfo.hasRollUpPluginInEtsLoader) return;
    const e = fse.existsSync(this.pathInfo.getSourceMapIntermediatePath()) ? this.pathInfo.getSourceMapIntermediatePath() : this.getSourceMapSourcePath();
    if (fse.existsSync(e)) if (hvigor_1.coreParameter.properties[const_1.OHOS_ARK_COMPILE_SOURCE_MAP_DIR]) {
      const t = this.getSourceMapPath(hvigor_1.coreParameter.properties[const_1.OHOS_ARK_COMPILE_SOURCE_MAP_DIR]);
      fse.ensureDirSync(path_1.resolve(t, this.moduleName)), fse.copyFileSync(e, path_1.resolve(t, this.moduleName, build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP));
    } else fse.ensureDirSync(this.pathInfo.getDefaultSourceMapPath()), fse.copyFileSync(e, path_1.resolve(this.pathInfo.getDefaultSourceMapPath(), build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP));
  }
  async replaceSourceMapInCoverageMode() {
    if (!inject_util_js_1.InjectUtil.isOhosTestCoverage()) return;
    const e = path_1.join(this.etsAssetsPath, build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP),
      t = path_1.join(this.pathInfo.getModulePath(), ".test/default/intermediates/ohosTest/coverage_sourcemap/ohosTestSourceMap.map");
    await (0, sourcemap_util_js_1.overWriteSourceMap)(e, t);
  }
  initTaskDepends() {
    this.declareDependsList(make_pack_info_js_1.MakePackInfo.name, cache_native_libs_js_1.CacheNativeLibs.name, generate_metadata_js_1.GenerateMetadata.name), this.declareDepends(syscap_transform_js_1.SyscapTransform.name), this.declareDepends(process_router_map_js_1.ProcessRouterMap.name), this.declareDepends(process_share_config_js_1.ProcessShareConfig.name), this.declareDepends(process_startup_config_js_1.ProcessStartupConfig.name), this.declareDepends(process_module_privacy_profile_js_1.ProcessModulePrivacyProfile.name);
    const e = fse.existsSync(path_1.resolve(this.sdkInfo.getEtsLoader(), sdk_const_js_1.ToolChainsConst.ROLL_UP_CONFIG_FILE)),
      t = fse.existsSync(path_1.resolve(this.sdkInfo.getJsLoader(), sdk_const_js_1.ToolChainsConst.ROLL_UP_CONFIG_FILE));
    if (this._log.debug("initTaskDepends:", `moduleName=${this.moduleName}, taskName=${this.name}, useRollupETS=${e}, useRollupJS=${t}`), inject_util_js_1.InjectUtil.isHotReload() || inject_util_js_1.InjectUtil.isColdReload()) return e && this.declareDepends(inject_util_js_1.InjectUtil.isHotReload() ? "HotReloadArkTS" : "ColdReloadArkTS"), void (t ? this.declareDepends("CompileJS") : this.declareDepends("BuildJS"));
    const s = this.targetName === ohosTestTargetName ? "OhosTest" : "";
    this.targetName === ohosTestTargetName && this.declareDepends(task_names_js_1.TaskNames.Task.OHOS_TEST_COPY_MOCK_CONFIG_JSON.name), this.declareDepends(this.sdkInfo.hasRollUpPluginInEtsLoader ? `${s}CompileArkTS` : `${s}BuildArkTS`), this.declareDepends(this.sdkInfo.hasRollUpPluginInJsLoader ? `${s}CompileJS` : `${s}BuildJS`);
  }
  getSourceMapPath(e) {
    return path_1.isAbsolute(e) ? e : path_1.resolve(this.projectModel.getProjectDir(), "hvigor", e);
  }
  doDependenciesCheck() {
    if (this.targetService.isDebug()) return;
    const {
      harDependencies: e,
      hspDependencies: t,
      byteCodeHarDependencies: s
    } = this.collectDependencies();
    this.doNativeStripDependenciesCheck(e), this.doObfuscationDependenciesCheck(t, s);
  }
  doNativeStripDependenciesCheck(e) {
    var t, s, o;
    if (false !== (null === (o = null === (s = null === (t = this.targetService.getBuildOption()) || undefined === t ? undefined : t.nativeLib) || undefined === s ? undefined : s.debugSymbol) || undefined === o ? undefined : o.strip)) return;
    const i = e.filter(e => {
      var t;
      if (e.isLocal()) return false;
      return false === (null === (t = e.getPackageJsonObj().metadata) || undefined === t ? undefined : t.nativeDebugSymbol);
    }).map(e => e.getPackageName());
    i.length && this._log.warn(`HAR dependencies [${i.join(", ")}] contain unstripped .so files. These .so files will not be stripped later.`);
  }
  excludeObfuscatedByteCodeHarDependencies(e) {
    const t = json_util_js_1.JsonUtil.getJson5Obj(this.pathInfo.getByteCodeObfConfigPath());
    if (!t) return e;
    const s = new Set(t.skippedRemoteHarList || []);
    return e.filter(e => s.has(e.getDependencyName()));
  }
  doObfuscationDependenciesCheck(e, t) {
    const s = e.filter(e => !e.isLocal()).filter(e => !e.getPackageJsonObj().obfuscated),
      o = this.excludeObfuscatedByteCodeHarDependencies(t).filter(e => !e.getPackageJsonObj().obfuscated);
    if (!s.length && !o.length) return;
    let i = "";
    o.length && (i += `Bytecode HARs [${o.map(e => e.getPackageName()).join(", ")}]`), s.length && (i += `${i ? " and " : ""}HSPs [${s.map(e => e.getPackageName()).join(", ")}]`), this._log.warn(`${i} to integrate are not obfuscated and will not be obfuscated later.`);
  }
  collectDependencies() {
    const e = new dependency_manager_js_1.DependencyManager(this.projectModel.isFaMode(), this.moduleModel, this.projectModel),
      {
        npmDependencies: t
      } = e.collectAllDependencies(),
      s = [],
      o = [],
      i = [];
    return t.forEach(e => {
      e.isByteCodeHarDependency() && i.push(e);
      const t = e.getDependencyType();
      t === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP ? s.push(e) : t === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR && o.push(e);
    }), {
      hspDependencies: s,
      harDependencies: o,
      byteCodeHarDependencies: i
    };
  }
  async beforeAlwaysAction() {
    if (await super.beforeAlwaysAction(), this.doDependenciesCheck(), (0, task_util_js_1.doObfuscationCheck)(this.targetService, this._log), inject_util_js_1.InjectUtil.isByteCodeHarOptimize()) {
      const e = this.getImporteesCachePath(hvigor_arkts_compose_1.DIRECT_IMPORTEE_CACHE_FILE_NAME, "CompileArkTS");
      this.doDevDependencyCheck(e);
    }
  }
  isAlignDeviceTypesContained(e) {
    const t = this.alignDeviceTypes;
    return !!t.length && t.every(t => !!(null == e ? undefined : e.includes(t)));
  }
  async copyIntermediates() {
    fse.mkdirsSync(this.fastAppDirPath);
    const e = [],
      t = path_1.resolve(this.fastAppDirPath, common_const_js_1.CommonConst.ETS),
      s = path_1.resolve(this.fastAppDirPath, common_const_js_1.CommonConst.RESOURCES_PATH),
      o = (0, integrated_hsp_utils_js_1.getAppInfoByProject)(this.service.getProjectModel()).bundleName;
    e.push(fse.copy(this.etsAssetsPath, t)), e.push(fse.copy(this.resourcePath, s)), e.push(fse.copyFile(this.indexPath, path_1.resolve(this.fastAppDirPath, build_directory_const_js_1.BuildArtifactConst.RESOURCE_INDEX))), fse.existsSync(this.pkgContextInfoPath) && this.useNormalizedOHMUrl && e.push(fse.copyFile(this.pkgContextInfoPath, path_1.resolve(this.fastAppDirPath, common_const_js_1.CommonConst.PKG_CONTEXT_INFO_JSON))), fse.existsSync(this.pkgSdkInfoPath) && !this.useNormalizedOHMUrl && e.push(fse.copyFile(this.pkgSdkInfoPath, path_1.resolve(this.fastAppDirPath, common_const_js_1.CommonConst.PKG_SDK_INFO_JSON)));
    const i = this.copyJSONFile(this.packageHapJsonPath, path_1.resolve(this.fastAppDirPath, common_const_js_1.CommonConst.MODULE_JSON), e => (e.app.bundleName = o, this.isAlignDeviceTypesContained(e.module.deviceTypes) && (e.module.deviceTypes = this.alignDeviceTypes), e)),
      a = this.copyJSONFile(this.allPackInfoPath[0], path_1.resolve(this.fastAppDirPath, build_directory_const_js_1.BuildArtifactConst.PACK_INFO), e => (e.summary.app.bundleName = o, this.alignDeviceTypes.length ? ([...e.summary.modules, ...e.packages].forEach(e => {
        this.isAlignDeviceTypesContained(e.deviceType) && (e.deviceType = this.alignDeviceTypes);
      }), e) : e));
    e.push(i), e.push(a), await Promise.all(e);
  }
  async copyJSONFile(e, t, s) {
    return fse.readFile(e).then(e => {
      let o = JSON.parse(e.toString());
      return o = s(o), fse.writeFile(t, JSON.stringify(o));
    }).catch(e => {
      this._log.debug("cannot copy intermediate file", e);
    });
  }
}
export const packageHapDepends = e => {
  const t = [make_pack_info_js_1.MakePackInfo.name, cache_native_libs_js_1.CacheNativeLibs.name, generate_metadata_js_1.GenerateMetadata.name],
    s = e.getTargetData(),
    o = e.getSdkInfo();
  if (t.push(syscap_transform_js_1.SyscapTransform.name), t.push(process_router_map_js_1.ProcessRouterMap.name), t.push(process_share_config_js_1.ProcessShareConfig.name), inject_util_js_1.InjectUtil.isHotReload() || inject_util_js_1.InjectUtil.isColdReload()) return o.hasRollUpPluginInEtsLoader && t.push(inject_util_js_1.InjectUtil.isHotReload() ? "HotReloadArkTS" : "ColdReloadArkTS"), t.push(o.hasRollUpPluginInJsLoader ? "CompileJS" : "BuildJS"), t;
  s.getTargetName() === ohosTestTargetName && t.push(task_names_js_1.TaskNames.Task.OHOS_TEST_COPY_MOCK_CONFIG_JSON.name);
  const i = s.getTargetName() === ohosTestTargetName ? "OhosTest" : "";
  return t.push(o.hasRollUpPluginInEtsLoader ? `${i}CompileArkTS` : `${i}BuildArkTS`), t.push(o.hasRollUpPluginInJsLoader ? `${i}CompileJS` : `${i}BuildJS`), t.push(process_integrated_hsp_js_1.ProcessIntegratedHsp.name), t;
};
