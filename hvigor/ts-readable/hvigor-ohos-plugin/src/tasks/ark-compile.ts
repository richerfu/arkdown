/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/ark-compile.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
import * as fse from "fs-extra";
import * as get_obfuscation_rules_js_1 from "../common/obfuscation/get-obfuscation-rules";
import * as ohos_trace_js_1 from "../common/trace/ohos-trace";
import * as type_js_1 from "../common/trace/type";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as inject_const_js_1 from "../const/inject-const";
import * as sdk_const_js_1 from "../const/sdk-const";
import * as code_type_enum_js_1 from "../enum/code-type-enum";
import * as custom_hook_service_js_1 from "../plugin/hooks/custom-hook-service";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
import * as build_profile_utils_js_1 from "../utils/build-profile-utils";
import * as byte_code_har_utils_js_1 from "../utils/byte-code-har-utils";
import global_store_js_1 from "../utils/data/global-store";
import * as dependency_util_js_1 from "../utils/dependency-util";
import * as file_util_js_1 from "../utils/file-util";
import * as har_target_util_js_1 from "../utils/har-target-util";
import * as inject_util_js_1 from "../utils/inject-util";
import * as json_util_js_1 from "../utils/json-util";
import * as oh_package_loader_js_1 from "../utils/loader/file/oh-package-loader";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as mock_config_utils_js_1 from "../utils/mock-config-utils";
import * as obfuscation_config_resolver_js_1 from "../utils/obfuscation-config-resolver";
import * as source_roots_utils_js_1 from "../utils/source-roots-utils";
import * as task_util_js_1 from "../utils/task-util";
import * as abstract_compile_node_js_1 from "./abstract/abstract-compile-node";
import * as task_names_js_1 from "./common/task-names";
import * as compile_resource_js_1 from "./compile-resource";
import * as generate_loader_json_js_1 from "./generate-loader-json";
import * as har_extend_info_js_1 from "./har/har-extend-info";
import * as ohos_uitransform_optimization_js_1 from "./module-info/ohos-uitransform-optimization";
import * as global_project_data_service_js_1 from "./service/global-project-data-service";
import * as target_task_service_js_1 from "./service/target-task-service";
import * as run_ark_js_1 from "./worker/run-ark";
import * as worker_id_helper_js_1 from "./worker/worker-id-helper";
var CommonTask = task_names_js_1.TaskNames.CommonTask;
export class ArkCompile extends abstract_compile_node_js_1.AbstractCompileNode {
  constructor(e, t, o) {
    super(e, t, o), this._log = ohos_logger_js_1.OhosLogger.getLogger(ArkCompile.name), this.obfuscationHash = "", this.localModuleParamMap = new Map(), this.needPreloadSoFileSet = new Set(), this.writeBuildModePromise = undefined, this.formJsonPathArr = target_task_service_js_1.TargetTaskService.getFormJsonArr(e), this.needSubmitArkTsWidget = (0, task_util_js_1.shouldCompileArkTsWidget)(this.formJsonPathArr, this.moduleModel.isFormExtensionModule(this.targetName)), this.pkgNameToPkgBriefInfo = {}, this._taskService = e, this.arkdataPathAll = [];
  }
  taskShouldDo() {
    return (undefined !== this.sourcePath || this.checkPackageMain()) && this.checkRollUpPlugin();
  }
  declareInputs() {
    var e, t, o, i, s, r, n, a, l, c, d, u, _, h, g, p, m, f, v, j, C;
    const S = super.declareInputs().set("arkTsWdiget", this.needSubmitArkTsWidget),
      y = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.DEBUG_LINE);
    y && S.set(common_const_js_1.CommonConst.DEBUG_LINE, y);
    const P = this.getObfuscationEnable();
    S.set(common_const_js_1.CommonConst.OBFUSCATION_ENABLE, String(P)), S.set(common_const_js_1.CommonConst.OBFUSCATION_FILES_HASH, this.obfuscationHash), S.set(common_const_js_1.CommonConst.TSC_MAX_FLOW_DEPTH, String(null === (o = null === (t = null === (e = this.buildOption) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.tscConfig) || undefined === o ? undefined : o.maxFlowDepth)), S.set(common_const_js_1.CommonConst.NO_EXTERNAL_IMPORT_BY_PATH, null !== (i = this.targetService.isNoExternalImportByPath()) && undefined !== i && i), S.set(common_const_js_1.CommonConst.COPY_CODE_RESOURCE_ENABLE, null === (a = null === (n = null === (r = null === (s = this.targetService.getBuildOption()) || undefined === s ? undefined : s.resOptions) || undefined === r ? undefined : r.copyCodeResource) || undefined === n ? undefined : n.enable) || undefined === a || a), (null === (u = null === (d = null === (c = null === (l = this.targetService.getBuildOption()) || undefined === l ? undefined : l.resOptions) || undefined === c ? undefined : c.copyCodeResource) || undefined === d ? undefined : d.enable) || undefined === u || u) && (S.set(common_const_js_1.CommonConst.COPY_CODE_RESOURCE_EXCLUDES, null !== (p = null === (g = null === (h = null === (_ = this.targetService.getBuildOption()) || undefined === _ ? undefined : _.resOptions) || undefined === h ? undefined : h.copyCodeResource) || undefined === g ? undefined : g.excludes) && undefined !== p ? p : []), S.set(common_const_js_1.CommonConst.COPY_CODE_RESOURCE_INCLUDES, null !== (j = null === (v = null === (f = null === (m = this.targetService.getBuildOption()) || undefined === m ? undefined : m.resOptions) || undefined === f ? undefined : f.copyCodeResource) || undefined === v ? undefined : v.includes) && undefined !== j ? j : "undefined")), S.set(common_const_js_1.CommonConst.OHOS_UI_TRANSFORM_OPTIMIZATION, (0, ohos_uitransform_optimization_js_1.getOhosUiTransformOptimization)(this.targetService)), S.set(common_const_js_1.CommonConst.OHOS_ROLLUP_CACHE_USE_SOURCE_HASH, true === hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.OHOS_ROLLUP_CACHE_USE_SOURCE_HASH)), S.set("localModuleParamMap", JSON.stringify(Object.fromEntries(this.localModuleParamMap)));
    const b = this.packageJsonObj;
    return S.set(common_const_js_1.CommonConst.OH_EXPORTS, JSON.stringify(null !== (C = b[common_const_js_1.CommonConst.OH_EXPORTS]) && undefined !== C ? C : [])), S;
  }
  declareOutputFiles() {
    const e = super.declareOutputFiles(),
      t = this.targetService.getTargetData().getTargetName();
    return !this.moduleModel.isHspModule() || t === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET || inject_util_js_1.InjectUtil.isColdReload() || inject_util_js_1.InjectUtil.isUnitTestProcess() || inject_util_js_1.InjectUtil.isHotReload() || inject_util_js_1.InjectUtil.isPreviewProcess() || e.addEntry(path_1.resolve(this.aceModuleBuild, `../${build_directory_const_js_1.BuildDirConst.DECLARE_FILE_OUTPUT}`), {
      isDirectory: true
    }), this.allowToPreloadSystemSo() && e.addEntry(this.pathInfo.getPreloadSoFilePath(), {
      isDirectory: false
    }), e;
  }
  addRootForDependencyCustomTypes(e, t) {
    const o = [],
      i = e.getDependencyRootPath(),
      s = e.getPackageJsonObj(),
      r = Object.keys({
        ...((null == s ? undefined : s.devDependencies) || {}),
        ...((null == s ? undefined : s.dependencies) || {})
      }),
      n = [],
      a = this.targetService.getCustomTypes();
    if (t.forEach(e => {
      if ([e, `@types/${e}`].some(e => r.includes(e))) return void ((null == a ? undefined : a.includes(e)) || n.push(e));
      !(0, build_profile_utils_js_1.getCustomTypePathIfExists)(i, e) ? n.push(e) : o.push(path_1.isAbsolute(e) ? e : path_1.resolve(i, e));
    }), n.length) {
      const e = (0, hvigor_common_1.getOsLanguage)(),
        t = (0, hvigor_logger_1.getUrl)(hvigor_logger_1.DEVELOPER_URL),
        o = `Can not find custom types: '[${n.join(",")}]', try to configure at file: ${this.service.getProjectModel().getProfilePath()}\n`;
      t ? this._log.warn(`${o}Reference: ${t}/consumer/${e}/doc/harmonyos-guides/arkoptions-guide#types`) : this._log.warn(`${o}Reference: Go to the official website to see Guides > Application Framework > ArkTS > ArkTS Compilation Toolchain > Configuring arkOptions in build-profile.json5`);
    }
    return o;
  }
  getDependencyCustomTypes(e, t) {
    var o, i;
    let s = [];
    if (e.isLocal()) {
      s = (null === (o = (0, dependency_util_js_1.getLocalDependencyBuildOptions)(e, t).arkOptions) || undefined === o ? undefined : o.types) || [];
    } else {
      s = (null === (i = e.getPackageJsonObj().metadata) || undefined === i ? undefined : i.customTypes) || [];
    }
    return this.addRootForDependencyCustomTypes(e, s);
  }
  collectCustomTypes() {
    const e = [...(this.targetService.getCustomTypes() || [])];
    if (!inject_util_js_1.InjectUtil.isDependenciesTypesEnable()) return e;
    const t = har_target_util_js_1.HarTargetUtil.calDepHarTargets(this.targetService);
    return this.service.getHarDependencies().forEach(o => {
      const i = this.getDependencyCustomTypes(o, t);
      i.length && e.push(...i);
    }), this._log.debug(`All custom types found in module '${this.moduleName}': ${JSON.stringify(e)}`), e;
  }
  addSelfPreloadSystemSoFileList() {
    var e, t;
    const o = null === (e = this._taskService.getTargetData().getModuleSourceSetModel().getModuleTargetRes().getModuleJsonOpt()) || undefined === e ? undefined : e.module;
    null === (t = null == o ? undefined : o.abilities) || undefined === t || t.forEach(e => {
      (0, task_util_js_1.isHomeAbility)(e) && this.needPreloadSoFileSet.add(path_1.resolve(this.pathInfo.getModuleSrcMainPath(), e.srcEntry));
    }), (null == o ? undefined : o.srcEntry) && this.needPreloadSoFileSet.add(path_1.resolve(this.pathInfo.getModuleSrcMainPath(), o.srcEntry));
  }
  addDependencyPreloadSystemSoFileList() {
    this.service.getHarDependencies().forEach(e => {
      var t, o, i;
      e.isByteCodeHarDependency() || null === (i = null === (o = null === (t = e.getModuleJsonObj()) || undefined === t ? undefined : t.module) || undefined === o ? undefined : o.abilities) || undefined === i || i.forEach(t => {
        (0, task_util_js_1.isHomeAbility)(t) && this.needPreloadSoFileSet.add(path_1.resolve(e.getDependencyRootPath(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, t.srcEntry));
      });
    });
  }
  allowToPreloadSystemSo() {
    return this.isPreloadSystemSoEnabled && !this.moduleModel.isHspModule();
  }
  addPreloadSystemSoFileList() {
    this.addSelfPreloadSystemSoFileList(), this.addDependencyPreloadSystemSoFileList();
  }
  async beforeAlwaysAction() {
    await super.beforeAlwaysAction(), this.obfuscationOptions = await this.getObfuscationOptions(), this.obfuscationOptions && (this.obfuscationHash = this.computeObfuscationFileHash(this.obfuscationOptions)), this._log.debug(`obfuscationOptions: ${JSON.stringify(this.obfuscationOptions)}`), this.arkDataCompile(), this.setPkgNameToPkgBriefInfo(), this.setByteCodeHarToDependencyKeys(), this.setLocalModuleParamMap(), this.userProjectConfig && (this.userProjectConfig.isPreloadSoEnabled = this.allowToPreloadSystemSo());
  }
  setByteCodeHarToDependencyKeys() {
    const e = inject_util_js_1.InjectUtil.isInTest(),
      t = !!this.obfuscationOptions,
      o = this.getUseNormalizedOHMUrl(),
      i = inject_util_js_1.InjectUtil.isColdReload() || inject_util_js_1.InjectUtil.isHotReload() || inject_util_js_1.InjectUtil.isPreviewProcess();
    !t || !o || this.moduleModel.isHarModule() || e || i || (this.byteCodeHarToDependencyKeys = (0, byte_code_har_utils_js_1.getByteCodeHarToDependencyKeys)(this.targetService));
  }
  setLocalModuleParamMap() {
    this.projectModel.getProject().getAllSubModules().forEach(e => {
      var t;
      const o = path_1.resolve(e.getNodeDir(), common_const_js_1.CommonConst.OH_PACKAGE_JSON5),
        i = oh_package_loader_js_1.ohPackageLoader.getOhPackageJsonObj(o),
        s = {
          parameterFilePath: this.projectModel.getParameterFileAbsolutePath(),
          ohPackageFilePath: o
        };
      if ((null == i ? undefined : i.version) && (null == i ? undefined : i.version.match(common_const_js_1.ParameterizationConst.REGEX))) {
        const o = file_util_js_1.FileUtil.extracted(null == i ? undefined : i.version, this.projectModel.getParameterFileObj(), s);
        null === (t = this.localModuleParamMap) || undefined === t || t.set(e.getNodeDir(), o);
      }
    });
  }
  async beforeTask() {
    var e;
    if (await super.beforeTask(), fse.existsSync(this.pathInfo.getPreloadSoFilePath()) && fse.removeSync(this.pathInfo.getPreloadSoFilePath()), !this.targetService.isDebug()) if (this.hasCacheBoosterPlugin()) await this.checkAndCleanCache();else {
      const t = this.getIncrementalCacheItem(common_const_js_1.CommonConst.OBFUSCATION_ENABLE);
      if (String(t) !== String(this.getObfuscationEnable())) return this._log.debug("Clean the arkts cache due to obfuscation config changes."), void (await this.cleanCache());
      if (this.getIncrementalCacheItem(common_const_js_1.CommonConst.OBFUSCATION_FILES_HASH) !== this.obfuscationHash) return this._log.debug("Clean the arkts cache due to obfuscation file changes."), void (await this.cleanCache());
      this.getIncrementalCacheItem(common_const_js_1.CommonConst.OH_EXPORTS) !== JSON.stringify(null !== (e = this.packageJsonObj[common_const_js_1.CommonConst.OH_EXPORTS]) && undefined !== e ? e : []) && (this._log.debug("Clean the arkts cache due to oh-exports changes."), await this.cleanCache());
    }
  }
  declareInputFiles() {
    var e, t;
    const o = super.declareInputFiles().addEntries(this.formJsonPathArr),
      i = this.targetService.getTargetData().getTargetName(),
      s = this.pathInfo.getGenerateBuildProfilePath(i);
    fse.existsSync(s) && o.addEntry(s);
    const r = this.targetService.getModuleService().getModuleModel().getProjectDir(),
      n = path_1.resolve(r, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, build_directory_const_js_1.BuildDirConst.CPP_TYPES);
    fse.existsSync(n) && o.addEntry(n, {
      isDirectory: true
    }), this.service.getHarModuleDependencies().forEach(([, e]) => {
      var t;
      const i = e.getDependencyRootPath(),
        s = path_1.resolve(i, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, build_directory_const_js_1.BuildDirConst.CPP_TYPES);
      fse.existsSync(s) && o.addEntry(s, {
        isDirectory: true
      });
      const r = path_1.resolve(i, build_directory_const_js_1.BuildArtifactConst.BUILD_PROFILE_FILE);
      fse.existsSync(r) && o.addEntry(r);
      const n = null === (t = this.pkgNameToPkgBriefInfo[e.getPackageName()]) || undefined === t ? undefined : t.originalSourceRoots;
      (null == n ? undefined : n.length) && n.forEach(e => {
        const t = path_1.resolve(i, e);
        fse.existsSync(t) && o.addEntry(path_1.resolve(i, e), {
          isDirectory: true
        });
      });
    });
    const a = null === (t = null === (e = this.targetService.getTargetData().getTargetOpt()) || undefined === e ? undefined : e.source) || undefined === t ? undefined : t.sourceRoots;
    (null == a ? undefined : a.length) && a.forEach(e => {
      o.addEntry(path_1.resolve(this.pathInfo.getModulePath(), e), {
        isDirectory: true
      });
    });
    const l = this.moduleModel.getMockConfigPath();
    fse.existsSync(l) && (o.addEntry(l, {
      isDirectory: false
    }), o.addEntries((0, mock_config_utils_js_1.getMockConfigSources)(l, this.moduleModel.getProjectDir())));
    const c = this.moduleModel.getCompilePluginPath();
    return c && fse.existsSync(c) && o.addEntry(c), o;
  }
  async doTaskAction() {
    this.validateModuleJson(this._log), this.allowToPreloadSystemSo() && this.addPreloadSystemSoFileList();
    const e = await this.initDefaultArkCompileConfig();
    if (e.entryModuleName && !global_store_js_1.get(e.entryModuleName)) {
      global_store_js_1.set(e.entryModuleName, true);
      const t = path_1.resolve(hvigor_1.PathUtil.getHvigorCacheDir(), "cache", "project-config.json");
      fse.ensureFileSync(t), fse.writeFileSync(t, JSON.stringify(e));
    }
    this._log.debug("build config:"), this._log.anonymizeDebug(e), e.externalApiPaths.length > 0 && this._log.debug(`Compile arkts with external api path: ${e.externalApiPaths.join(path_1.delimiter)}`), e.obfuscationOptions && (await fse.ensureDir(e.obfuscationOptions.obfuscationCacheDir), await fse.ensureDir(path_1.dirname(e.obfuscationOptions.exportRulePath))), e.cachePath && this.trace(e.cachePath), fse.ensureDirSync(e.cachePath), fse.ensureDirSync(e.aceModuleBuild);
    let t = null,
      o = hvigor_1.coreParameter.properties.ohosArkCompileMaxSize;
    (undefined === o || o <= 0 || o > hvigor_1.PoolConstant.MAX_POOL_NUM) && (o = common_const_js_1.ArkPackConst.MAX_ARK_COMPILE_NUM);
    const i = [...new Array(o).keys()];
    let s = i;
    const r = (0, worker_id_helper_js_1.getWorkerIdWithModule)(e.modulePath),
      n = this.getWorkerPool().getMaxPoolNum();
    if (undefined !== r && n > 0 && (s = [(0, hvigor_common_1.mod)(r, Math.min(n, o))]), t = await (0, run_ark_js_1.submitArkCompileWork)(this, this.getWorkerPool(), this._log, e, async (o, i) => {
      undefined !== i && (0, worker_id_helper_js_1.setWorkerIdWithModule)(e.modulePath, i), this.moveReleaseMap(this.targetData, this._log, this.codeType), this.handleCompileEvents(o, t ? this.getSubDurationEvent(t) : this.durationEvent, false), await this.writeBuildMode(e);
    }, s, this.getTransformAbcAction() ? this.doBeforeObfuscationAction.bind(this) : undefined), this.needSubmitArkTsWidget) {
      const t = await this.initWidgetDefaultArkCompileConfig();
      let s = i;
      const r = (0, worker_id_helper_js_1.getWorkerIdWithModule)(e.modulePath, true);
      undefined !== r && n > 0 && (s = [(0, hvigor_common_1.mod)(r, Math.min(n, o))]), t.ignoreWarning = true;
      let a = null;
      a = await (0, run_ark_js_1.submitArkCompileWork)(this, this.getWorkerPool(), this._log, t, async (o, i) => {
        undefined !== i && (0, worker_id_helper_js_1.setWorkerIdWithModule)(e.modulePath, i, true), this.handleCompileEvents(o, a ? this.getSubDurationEvent(a) : this.durationEvent, false), await this.writeBuildMode(t);
      }, s);
    }
    (0, task_util_js_1.warnCreateBuildProfileTask)(this.targetData, this.targetService, this._log);
  }
  getTransformAbcAction() {
    return custom_hook_service_js_1.customHookActionService.getCustomAction(this.module.getNodeDir(), hvigor_arkts_compose_1.TRANSFORM_ABC_WORKER_REQUEST, this.targetName);
  }
  doBeforeObfuscationAction(e) {
    var t;
    return null === (t = this.getTransformAbcAction()) || undefined === t ? undefined : t(e.abcPath, e.config);
  }
  async writeBuildMode(e) {
    if (!this.writeBuildModePromise) {
      const t = path_1.resolve(e.arkCompileCachePath, "compileInfo.json");
      this.writeBuildModePromise = new Promise(o => {
        e.buildMode && (fse.ensureFileSync(t), fse.writeJsonSync(t, {
          buildMode: e.buildMode
        }), o());
      }).finally(() => {
        this.writeBuildModePromise = undefined;
      });
    }
    await this.writeBuildModePromise;
  }
  trace(e) {
    const t = path_1.resolve(e, hvigor_arkts_compose_1.MSGPACK_COMPILE_CACHE_DIR_NAME);
    ohos_trace_js_1.ohosTrace.traceIncrement(this.moduleName, type_js_1.TraceIncrement.COMPILE_ARKTS, fse.existsSync(t));
  }
  getSelfPkgBriefInfo() {
    var e;
    const t = null === (e = this.targetData.getTargetOpt().source) || undefined === e ? undefined : e.sourceRoots,
      o = this.pathInfo.getModulePath(),
      i = this.packageJsonObj.name || this.moduleName,
      s = {
        pkgRoot: o,
        originalSourceRoots: t,
        sourceRoots: [...(t || []), path_1.relative(o, this.pathInfo.getModuleSrcMainPath()), path_1.relative(o, path_1.resolve(this.pathInfo.getGenerateBuildProfileDir(), this.targetName))],
        pkgName: i
      };
    return {
      [i]: s
    };
  }
  setPkgNameToPkgBriefInfo() {
    this.pkgNameToPkgBriefInfo = {
      ...(0, source_roots_utils_js_1.getDependenciesPkgBriefInfo)(this.targetService),
      ...this.getSelfPkgBriefInfo()
    };
  }
  getCollectImportersConfig() {
    var e, t;
    const o = this.pathInfo.getBuildConfigPath(),
      i = json_util_js_1.JsonUtil.getJson5Obj(o);
    if ("hotReload" === (null === (e = null == i ? undefined : i.patchConfig) || undefined === e ? undefined : e.mode) || "coldReload" === (null === (t = null == i ? undefined : i.patchConfig) || undefined === t ? undefined : t.mode)) return {
      relativeDir: path_1.resolve(this.pathInfo.getModuleSrcMainPath(), "ets"),
      buildConfigJsonPath: this.pathInfo.getBuildConfigPath(),
      isReload: false,
      patchPath: this.pathInfo.getPatchDir()
    };
  }
  getTscAddressPaths(e) {
    const t = this.pkgNameToPkgBriefInfo,
      o = {};
    return Object.values(t).forEach(t => {
      var i;
      const s = null === (i = t.sourceRoots) || undefined === i ? undefined : i.map(o => path_1.join(path_1.relative(e, path_1.resolve(t.pkgRoot, o)), "*"));
      (null == s ? undefined : s.length) && (o[`${t.pkgName}/*`] = s);
    }), o;
  }
  async initDefaultArkCompileConfig() {
    var e, t, o, i, s, r, n, a, l, c, d, u, _, h;
    const g = this.targetData.getModuleModel().getMockConfigPath(),
      p = {
        ...(await super.initDefaultArkCompileConfig()),
        pkgNameToPkgBriefInfo: this.pkgNameToPkgBriefInfo,
        projectModel: global_project_data_service_js_1.GlobalProjectDataService.getInstance().getCompileProjectModel(),
        pkgJsonFileHash: global_project_data_service_js_1.GlobalProjectDataService.getInstance().getProjectPkgJsonFileHash(),
        allModulePaths: global_project_data_service_js_1.GlobalProjectDataService.getInstance().getAllModulePaths(),
        routerMap: this.getRouterMapConfig(),
        obfuscationOptions: this.obfuscationOptions,
        byteCodeHarToDependencyKeys: this.byteCodeHarToDependencyKeys,
        compileBlockPkg: this.getBlockDependencies().map(e => e.getDependencyName()),
        mockParams: {
          decorator: "@MockSetup",
          packageName: "@ohos/hamock",
          etsSourceRootPath: "src/main/ets",
          mockConfigPath: fse.existsSync(g) ? g : undefined,
          mockConfigKey2ModuleInfo: (0, mock_config_utils_js_1.getMockConfigKey2ModuleInfoMap)(this.moduleModel.getMockConfigPath(), this.service),
          source2ModuleIdMap: (0, mock_config_utils_js_1.getSource2ModuleIdMap)(g, this.moduleModel.getProjectDir())
        },
        caseSensitiveCheck: !(!(0, hvigor_1.isWindows)() && !(0, hvigor_1.isMac)()) && null !== (o = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.strictMode) || undefined === t ? undefined : t.caseSensitiveCheck) && undefined !== o && o,
        copyCodeResourceEnable: null === (n = null === (r = null === (s = null === (i = this.targetService.getBuildOption()) || undefined === i ? undefined : i.resOptions) || undefined === s ? undefined : s.copyCodeResource) || undefined === r ? undefined : r.enable) || undefined === n || n,
        copyCodeResourceExcludes: null !== (d = null === (c = null === (l = null === (a = this.targetService.getBuildOption()) || undefined === a ? undefined : a.resOptions) || undefined === l ? undefined : l.copyCodeResource) || undefined === c ? undefined : c.excludes) && undefined !== d ? d : [],
        copyCodeResourceIncludes: null === (h = null === (_ = null === (u = this.targetService.getBuildOption()) || undefined === u ? undefined : u.resOptions) || undefined === _ ? undefined : _.copyCodeResource) || undefined === h ? undefined : h.includes,
        uiTransformOptimization: (0, ohos_uitransform_optimization_js_1.getOhosUiTransformOptimization)(this.targetService),
        sdkPath: this.sdkInfo.sdkDir,
        needPreloadSoFileSet: this.needPreloadSoFileSet,
        preloadSoFilePath: this.pathInfo.getPreloadSoFilePath(),
        useSourceHash: true === hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.OHOS_ROLLUP_CACHE_USE_SOURCE_HASH),
        localModuleParamMap: this.localModuleParamMap
      };
    return p.aceModuleRoot || (p.aceModuleRoot = path_1.resolve(this.pathInfo.getModuleSrcMainPath(), this.codeType)), p.otherPaths = this.getTscAddressPaths(p.aceModuleRoot), p.collectImportersConfig = this.getCollectImportersConfig(), p;
  }
  async initWidgetDefaultArkCompileConfig() {
    const e = this.getTaskTempDir(this.targetData, "_widget"),
      t = {
        ...(await this.initDefaultArkCompileConfig()),
        widgetCompile: "true",
        cachePath: e,
        aceBuildJson: this.pathInfo.getWidgetLoaderJsonPath(),
        byteCodeHar2HspDep: this.service.getByteCodeHar2Hsp()
      };
    return t.obfuscationOptions && (t.obfuscationOptions = {
      ...t.obfuscationOptions,
      obfuscationCacheDir: path_1.resolve(e, "obfuscation")
    }), t;
  }
  getRouterMapConfig() {
    var e;
    const t = new Map(),
      o = [];
    this.service.getHarDependencies().forEach(e => {
      if (e.isLocal()) {
        const t = (0, dependency_util_js_1.getLocalDependencyRouterMap)(this.projectModel, this.targetService, e);
        t && Array.prototype.push.apply(o, t);
      } else {
        const t = dependency_manager_js_1.DependencyManager.getRemoteDependencyRouterMapObjList(e);
        t && Array.prototype.push.apply(o, t);
      }
    });
    const i = null !== (e = this.targetService.getTargetRouterMapObjList(this.moduleModel)) && undefined !== e ? e : o;
    return null == i || i.sort((e, t) => e.name.localeCompare(t.name)).forEach(e => {
      const o = path_1.resolve(e.moduleNodeDir, e.pageSourceFile),
        i = `${e.name}@${e.buildFunction}`,
        s = t.has(o) ? `${t.get(o)}#${i}` : i;
      t.set(o, s);
    }), t;
  }
  arkDataCompile() {
    const e = this._taskService.getModuleService().getHarDependencies();
    if (undefined === e || e.length <= 0) return;
    const t = this._taskService.getModuleService().getModuleModel().getName(),
      o = this._taskService.getTargetData().getPathInfo().getModuleSrcMainResourceRawfileArkdataPath(),
      i = this._taskService.getTargetData().getPathInfo().getIntermediatesResArkDataPath();
    this.arkdataPathAll.push(o);
    const s = new Set();
    if (fse.existsSync(o)) {
      const e = fse.readdirSync(o);
      for (const t of e) s.add(t);
    }
    fse.existsSync(i) || fse.mkdirSync(i, {
      recursive: true
    });
    for (const o of e) {
      const e = o.getPackageName(),
        r = this._taskService.getModuleService().getProjectModel().getModuleModelByName(e),
        n = null == r ? undefined : r.getProjectDir();
      if (undefined === n) continue;
      const a = path_1.resolve(n, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, build_directory_const_js_1.BuildDirConst.RESOURCES, build_directory_const_js_1.BuildDirConst.RAW_FILE, build_directory_const_js_1.BuildDirConst.ARK_DATA, build_directory_const_js_1.BuildDirConst.SCHEMA);
      if (!fse.existsSync(a)) continue;
      this.arkdataPathAll.push(a);
      const l = fse.readdirSync(a);
      for (const o of l) {
        s.has(o) && this._log.printErrorExit("SAME_NAME_ARK_DATA_JSON_FILE", [o, e, o, t]), s.add(o);
        const r = path_1.resolve(a, o),
          n = path_1.resolve(i, o);
        fse.existsSync(i) && fse.copyFileSync(r, n);
      }
    }
  }
  initTaskDepends() {
    if ((0, task_util_js_1.limitMinApiVersion)(this.targetData, sdk_const_js_1.ApiVersion.API_VERSION_9)) {
      this.declareDependsList(generate_loader_json_js_1.GenerateLoaderJson.name, compile_resource_js_1.CompileResource.name, CommonTask.CREATE_BUILD_PROFILE.name);
      har_target_util_js_1.HarTargetUtil.calDepHarTargets(this.targetService).forEach((e, t) => this.declareDepends(`${e}@${CommonTask.CREATE_HAR_BUILD_PROFILE.name}`, t));
    } else this.declareDependsList(generate_loader_json_js_1.GenerateLoaderJson.name, compile_resource_js_1.CompileResource.name);
  }
  async getObfuscationOptions() {
    var e, t, o, i;
    const s = this.targetService.getBuildOption();
    if (s.debuggable) return void ("true" === (null === (e = hvigor_1.hvigorCore.getExtraConfig().get(inject_const_js_1.InjectConst.ARK_OBFUSCATION)) || undefined === e ? undefined : e.toLowerCase()) && this._log.warn("Obfuscation is only enabled in release mode."));
    const r = path_1.resolve(this.getTaskTempDir(this.targetData), "obfuscation");
    let n = null === (t = s.arkOptions) || undefined === t ? undefined : t.obfuscation;
    s.artifactType && this._log.warn("buildOption/artifactType is deprecated, please use buildOption/arkOptions/obfuscation instead."), s.artifactType === har_extend_info_js_1.ArtifactType.OBFUSCATION && undefined === (null === (o = null == n ? undefined : n.ruleOptions) || undefined === o ? undefined : o.enable) && (n = {
      ...n,
      ruleOptions: {
        enable: true
      },
      libDir: this.module.getNodeDir()
    });
    const a = this.getObfuscationEnable();
    if (!n || undefined === (null === (i = n.ruleOptions) || undefined === i ? undefined : i.enable)) {
      if (undefined === a) return;
      this._log.debug(`Command line specified enable was used, obfuscationEnable is: ${a}`), n = {
        ruleOptions: {
          enable: a
        },
        libDir: this.module.getNodeDir()
      };
    }
    if (!(this.compileApiVersion < sdk_const_js_1.ApiVersion.API_VERSION_10 || this.isFaMode)) {
      if (this.moduleModel.isHapModule() && undefined !== n.consumerFiles && this._log.warn("The property 'consumerFiles' is not support in hap module´s build-profile.json5"), undefined !== a && n.ruleOptions && (n.ruleOptions.enable = a), true === hvigor_1.hvigorCore.getParameter().getPropertyInternal("ohos.obfuscationRules.optimization")) try {
        return (0, get_obfuscation_rules_js_1.getObfuscationOptionsWithCache)(this.targetService, n, r);
      } catch (e) {
        return (0, obfuscation_config_resolver_js_1.resolveObfuscationOptions)(this.targetService, n, r);
      }
      return (0, obfuscation_config_resolver_js_1.resolveObfuscationOptions)(this.targetService, n, r);
    }
    this._log.warn(`Obfuscation is supported in stage mode of API${sdk_const_js_1.ApiVersion.API_VERSION_10} or later.`);
  }
  computeObfuscationFileHash(e) {
    var t, o, i, s, r, n, a, l;
    const c = [];
    c.push(...(null !== (i = null === (o = null === (t = e.selfConfig) || undefined === t ? undefined : t.ruleOptions) || undefined === o ? undefined : o.rules) && undefined !== i ? i : [])), c.push(...(null !== (r = null === (s = e.selfConfig) || undefined === s ? undefined : s.consumerRules) && undefined !== r ? r : [])), c.push(...e.dependencies.hars), c.push(...e.dependencies.hsps);
    for (const t of [...e.dependencies.libraries, ...e.dependencies.hspLibraries]) c.push(...(null !== (a = null === (n = t.ruleOptions) || undefined === n ? undefined : n.rules) && undefined !== a ? a : [])), c.push(...(null !== (l = t.consumerRules) && undefined !== l ? l : []));
    const d = (0, hvigor_common_1.createHash)();
    for (const e of c) fse.existsSync(e) && d.update(fse.readFileSync(e));
    return d.digest("hex");
  }
  getIncrementalCacheItem(e) {
    var t, o, i;
    return null === (i = null === (o = null === (t = hvigor_1.ProjectCacheService.getInstance(this.projectModel.getProject()).getTaskSnapShot(this.getName(), this.module)) || undefined === t ? undefined : t.getInputs()) || undefined === o ? undefined : o.find(t => t.getName() === e)) || undefined === i ? undefined : i.getValue();
  }
  async cleanCache() {
    const e = this.getTaskTempDir(this.targetData);
    await fse.emptydir(e);
    const t = path_1.resolve(e, "..", ".ts_checker_cache");
    fse.existsSync(t) && (await fse.rm(t));
  }
  getObfuscationEnable() {
    var e, t, o, i, s;
    const r = null === (e = hvigor_1.hvigorCore.getExtraConfig().get(inject_const_js_1.InjectConst.ARK_OBFUSCATION)) || undefined === e ? undefined : e.toLowerCase();
    return "false" !== r && ("true" === r || (null === (s = null === (i = null === (o = null === (t = this.buildOption) || undefined === t ? undefined : t.arkOptions) || undefined === o ? undefined : o.obfuscation) || undefined === i ? undefined : i.ruleOptions) || undefined === s ? undefined : s.enable));
  }
  checkPackageMain() {
    const e = this.packageJsonObj.main ? this.packageJsonObj.main : build_directory_const_js_1.BuildArtifactConst.INDEX_ETS;
    return fse.existsSync(path_1.resolve(this.pathInfo.getModulePath(), e));
  }
  checkRollUpPlugin() {
    return this.codeType === code_type_enum_js_1.CodeType.ETS ? this.sdkInfo.hasRollUpPluginInEtsLoader : this.sdkInfo.hasRollUpPluginInJsLoader;
  }
  onFailed(e) {
    super.onFailed(e);
    const t = e;
    t.extraInfo && (t.extraInfo.infoMsg && this._log.info(t.extraInfo.infoMsg), t.extraInfo.warnMsg && this._log.warn(t.extraInfo.warnMsg));
  }
  async checkAndCleanCache() {
    const e = path_1.resolve(this.getTaskTempDir(this.targetData), ArkCompile.TASK_COMPILE_ARK_TS_FILE_NAME),
      t = String(this.getObfuscationEnable()),
      o = this.obfuscationHash;
    let i, s;
    try {
      if (await fse.pathExists(e)) {
        const t = await fse.readFile(e, "utf-8"),
          o = JSON.parse(t);
        i = o.enable, s = o.hash, "undefined" === s && (s = undefined);
      }
    } catch (e) {
      this._log.debug("read obfuscation cache file error: ", e);
    }
    t !== i && (this._log.debug("Clean the arkts cache due to obfuscation config changes."), await this.cleanCache()), o !== s && (this._log.debug("Clean the arkts cache due to obfuscation file changes."), await this.cleanCache()), await fse.writeFile(e, JSON.stringify({
      enable: t,
      hash: o
    }), "utf-8");
  }
  hasCacheBoosterPlugin() {
    const e = this.targetService.getModuleService().getNode().getAllPluginIds();
    for (const t of e) if ("cache-booster" === t) return true;
    return false;
  }
}
ArkCompile.TASK_COMPILE_ARK_TS_FILE_NAME = "TaskCompileArkTS.cache";
