/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/compile-resource.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as common_util_js_1 from "../common/common-util";
import * as project_ohos_config_manager_js_1 from "../common/global/project-ohos-config-manager";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as distro_filter_handler_js_1 from "../distribution/distro-filter-handler";
import * as compile_resources_util_js_1 from "../utils/compile-resources-util";
import * as file_util_js_1 from "../utils/file-util";
import * as res_model_loader_js_1 from "../utils/loader/file/res-model-loader";
import * as json_util_js_1 from "../utils/json-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as abstract_compile_resource_js_1 from "./abstract/abstract-compile-resource";
import * as task_names_js_1 from "./common/task-names";
import * as process_resource_js_1 from "./process-resource";
var Task = task_names_js_1.TaskNames.Task;
export class CompileResource extends abstract_compile_resource_js_1.AbstractCompileResource {
  async beforeAlwaysAction() {
    return this.checkStartupTaskDependencies(), super.beforeAlwaysAction();
  }
  declareInputs() {
    var e, t;
    const s = super.declareInputs(),
      o = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.strictMode) || undefined === t ? undefined : t.useNormalizedOHMUrl;
    return s.set("useNormalizedOHMUrl", !!o), s;
  }
  declareInputFiles() {
    var e, t, s, o;
    const i = this.restoolConfigBuilder.inputFiles,
      r = this.pathInfo.getIntermediatesRouterMap(null !== (e = this.targetService.getRouterMapJsonFileName(this.moduleModel)) && undefined !== e ? e : build_directory_const_js_1.BuildArtifactConst.TEMP_ROUTER_MAP_FILE_NAME);
    if (fs_extra_1.existsSync(r)) {
      (null === (t = json_util_js_1.JsonUtil.getJson5Obj(r).routerMap) || undefined === t ? undefined : t.length) && i.addEntry(r);
    }
    const a = this.pathInfo.getIntermediatesShareConfig(null !== (s = this.targetService.getShareConfigJsonFileName(this.moduleModel)) && undefined !== s ? s : build_directory_const_js_1.BuildArtifactConst.TEMP_SHARE_CONFIG_FILE_NAME);
    if (fs_extra_1.existsSync(a)) {
      const e = json_util_js_1.JsonUtil.getJson5Obj(a);
      (null === (o = null == e ? undefined : e.crossAppSharedConfig) || undefined === o ? undefined : o.length) && i.addEntry(a);
    }
    return this.intermediateTempStartupFilePath && fs_extra_1.existsSync(this.intermediateTempStartupFilePath) && i.addEntry(this.intermediateTempStartupFilePath), i.addEntries([this.resConfigFilePath], {
      isDirectory: false
    }), this.shouldCompileResourceForHarDeduplicate() && i.addEntry(this.resConfigFilePathForHarDeduplicate), i;
  }
  declareOutputFiles() {
    const e = super.declareOutputFiles();
    return this.shouldCompileResourceForHarDeduplicate() && e.addEntry(this.pathInfo.getIntermediatesResDeduplicate()), e;
  }
  declareExecutionCommand() {
    return this.getCommand().toString();
  }
  constructor(e) {
    super(e, Task.COMPILE_RESOURCE), this._log = ohos_logger_js_1.OhosLogger.getLogger(CompileResource.name), this.generateSourceRDir = this.pathInfo.getGenerateSourceR(), this.resConfigFilePath = path_1.resolve(this.pathInfo.getIntermediatesRes(), common_const_js_1.CommonConst.RES_CONFIG_FILE), this.resConfigFilePathForHarDeduplicate = path_1.resolve(this.pathInfo.getIntermediatesResDeduplicate(), common_const_js_1.CommonConst.RES_CONFIG_FILE), this.processProfileTask = process_resource_js_1.ProcessResource.name;
  }
  initTaskDepends() {
    this.declareDepends(process_resource_js_1.ProcessResource.name);
  }
  async doTaskAction() {
    const e = [];
    e.push(this.compilationProcess(compile_resources_util_js_1.CompileResourceBuildCommandType.FILE, this.targetData)), this.shouldCompileResourceForHarDeduplicate() && e.push(this.compileResourcesForHarDeduplicate()), await Promise.all(e);
  }
  async invokeRestool(e) {
    const t = "create intermediate resource category",
      s = this.durationEvent.createSubEvent(t, "");
    s.start(), await fs_extra_1.ensureDir(path_1.resolve(this.pathInfo.getIntermediatesRes(), "ids_map")), s.stop(), s.setLog(t, hvigor_1.MetricLogType.INFO);
    const o = this.getRestoolEnv(),
      i = path_1.dirname(this.getResToolFile());
    await this.executeCommand(this.getCommand(), this._log, () => this.replaceTargetPages(e, this._log), {
      env: o,
      cwd: i
    }), this.customizeDistroFilter(), this.customizeShortcut(), this.processRouterMap(), this.processShareConfig(), this.processStartupConfig(), this.processFormExtensionAbilities();
  }
  getRestoolEnv() {
    const e = this.sdkInfo.getHosToolchainsLibDir(),
      t = this.sdkInfo.getPreviewerCommonBinDir(),
      s = {
        path: `${e};${t}`
      },
      o = {
        DYLD_FALLBACK_LIBRARY_PATH: `${process.env.DYLD_FALLBACK_LIBRARY_PATH}:${e}:${t}`
      },
      i = {
        LD_LIBRARY_PATH: `${process.env.LD_LIBRARY_PATH}:${e}:${t}`
      };
    return (0, hvigor_1.isWindows)() ? s : (0, hvigor_1.isLinux)() ? i : o;
  }
  async compileResourcesForHarDeduplicate() {
    if (this.precheck(compile_resources_util_js_1.CompileResourceBuildCommandType.FILE)) {
      const e = this.getRestoolEnv(),
        t = this.getResToolFile(),
        s = path_1.dirname(t);
      await this.executeCommand([t, "-l", this.resConfigFilePathForHarDeduplicate], this._log, wdk_1.noop, {
        env: e,
        cwd: s
      });
    }
  }
  customizeDistroFilter() {
    var e, t;
    const s = this.targetData.getTargetOpt();
    if (!(null === (e = s.config) || undefined === e ? undefined : e.distributionFilter)) return;
    const o = s.config.distributionFilter,
      i = this.service.getProjectModel().getProjectDir(),
      r = this.targetData.getApiMeta().compileSdkVersion.version,
      a = this.service.getModuleModel(),
      l = a.getModule().getNodeDir(),
      n = path_1.resolve(i, l, `src/main/${common_const_js_1.CommonConst.MODULE_JSON5}`),
      u = json_util_js_1.JsonUtil.getJson5Obj(n).module.metadata;
    let d,
      _ = {};
    const c = distro_filter_handler_js_1.DistroFilterHandler.findFirstDistroFilterConfigMetadata(i, a, u);
    let h = "";
    if (9 === r ? h = "distroFilter" : r >= 10 && (h = "distributionFilter"), !c) {
      const e = path_1.resolve(this.pathInfo.getModuleProductPath(), `intermediates/res/${s.name}/${build_directory_const_js_1.BuildDirConst.RESOURCES_BASE_PROFILE_DIR}`),
        t = file_util_js_1.FileUtil.traverseFileFolder(e);
      d = file_util_js_1.FileUtil.uniqueFileName(t, "ohos_module_distro_filter");
      const i = path_1.resolve(this.pathInfo.getModuleProductPath(), `intermediates/res/${s.name}/module.json`),
        r = json_util_js_1.JsonUtil.getJson5Obj(i);
      r.module.metadata ? r.module.metadata.push({
        resource: `$profile:${d}`
      }) : Object.assign(r.module, {
        metadata: [{
          resource: `$profile:${d}`
        }]
      }), fs_extra_1.writeJSONSync(i, r), _ = {
        [h]: o
      };
    }
    if (null == c ? undefined : c.resource) {
      d = (0, common_util_js_1.parsingProfileName)(c.resource);
      const e = path_1.resolve(i, l, `src/main/${build_directory_const_js_1.BuildDirConst.RESOURCES_BASE_PROFILE_DIR}/${d}.json`),
        s = json_util_js_1.JsonUtil.getJson5Obj(e);
      h = null !== (t = Object.keys(s)[0]) && undefined !== t ? t : h, _ = {
        [h]: {
          ...s.distroFilter,
          ...o
        }
      };
    }
    const p = path_1.resolve(this.pathInfo.getModuleProductPath(), `intermediates/res/${s.name}/${build_directory_const_js_1.BuildDirConst.RESOURCES_BASE_PROFILE_DIR}/${d}.json`);
    if (fs_extra_1.existsSync(p) || fs_extra_1.createFileSync(p), fs_extra_1.writeJSONSync(p, _), this.shouldCompileResourceForHarDeduplicate()) {
      const e = path_1.resolve(this.pathInfo.getIntermediatesResDeduplicateProfilePath(), `${d}.json`);
      fs_extra_1.outputJSONSync(e, _);
    }
  }
  customizeShortcut() {
    const e = this.targetData.getProduct(),
      t = this.moduleModel.getModule().getNodeDir(),
      s = path_1.resolve(t, `src/main/${common_const_js_1.CommonConst.MODULE_JSON5}`),
      o = res_model_loader_js_1.resModelLoader.getModuleJson(s).module.metadata,
      i = this.projectModel.getAppRes().getAppResOpt().app.bundleName;
    o && 0 !== o.length && o.forEach(s => {
      if (!s.resource) return;
      const o = (0, common_util_js_1.parsingProfileName)(s.resource),
        r = path_1.resolve(t, `src/main/${build_directory_const_js_1.BuildDirConst.RESOURCES_BASE_PROFILE_DIR}/${o}.json`),
        a = json_util_js_1.JsonUtil.getJson5Obj(r).shortcuts;
      if (!a || 0 === a.length) return;
      const l = this.targetData.getTargetOpt().name,
        n = path_1.resolve(this.pathInfo.getModuleProductPath(), `intermediates/res/${l}/${build_directory_const_js_1.BuildDirConst.RESOURCES_BASE_PROFILE_DIR}/${o}.json`),
        u = path_1.resolve(this.pathInfo.getIntermediatesResDeduplicateProfilePath(), `${o}.json`);
      a.forEach(t => {
        this.processShortcutObjBundleName(t, i, e, n, u);
      });
    });
  }
  processShortcutObjBundleName(e, t, s, o, i) {
    const r = e,
      a = r.wants;
    a && 0 !== a.length && (a.forEach(e => {
      var o;
      if (t === e.bundleName) {
        const t = (0, wdk_1.cloneDeep)(null === (o = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === o ? undefined : o.appOpt);
        if (null == t ? undefined : t.bundleName) return void (e.bundleName = t.bundleName);
        s.bundleName && (e.bundleName = s.bundleName);
      }
    }), fs_extra_1.existsSync(o) || fs_extra_1.createFileSync(o), fs_extra_1.writeJSONSync(o, {
      newShortCutObj: r
    }), this.shouldCompileResourceForHarDeduplicate() && fs_extra_1.outputJSONSync(i, {
      newShortCutObj: r
    }));
  }
  writeShareConfig(e) {
    var t;
    if (!e || 0 === e.length) return;
    const s = null !== (t = this.targetService.getShareConfigJsonFileName(this.moduleModel)) && undefined !== t ? t : build_directory_const_js_1.BuildArtifactConst.DEFAULT_SHARE_CONFIG,
      o = path_1.resolve(this.targetData.getPathInfo().getIntermediatesResProfilePath(), `${s}.json`);
    if (fs_extra_1.ensureFileSync(o), fs_extra_1.writeFileSync(o, JSON.stringify({
      crossAppSharedConfig: e
    })), this.shouldCompileResourceForHarDeduplicate()) {
      const t = path_1.resolve(this.pathInfo.getIntermediatesResDeduplicateProfilePath(), `${s}.json`);
      fs_extra_1.outputJSONSync(t, {
        crossAppSharedConfig: e
      });
    }
    const i = path_1.resolve(this.targetData.getPathInfo().getIntermediatesRes(), common_const_js_1.CommonConst.MODULE_JSON),
      r = json_util_js_1.JsonUtil.getJson5Obj(i);
    Object.assign(r.module, {
      crossAppSharedConfig: `$profile:${s}`
    }), fs_extra_1.writeFileSync(i, JSON.stringify(r));
  }
  processShareConfig() {
    var e;
    const t = this.pathInfo.getIntermediatesShareConfig(null !== (e = this.targetService.getShareConfigJsonFileName(this.moduleModel)) && undefined !== e ? e : build_directory_const_js_1.BuildArtifactConst.TEMP_SHARE_CONFIG_FILE_NAME);
    if (!fs_extra_1.existsSync(t)) return;
    const s = json_util_js_1.JsonUtil.getJson5Obj(t),
      o = null == s ? undefined : s.crossAppSharedConfig;
    (null == o ? undefined : o.length) && (o.forEach(e => {
      delete e.moduleNodeDir, delete e.packageName;
    }), this.writeShareConfig(o));
  }
  writeRouterMap(e) {
    var t;
    if (!e || 0 === e.length) return;
    const s = null !== (t = this.targetService.getRouterMapJsonFileName(this.moduleModel)) && undefined !== t ? t : build_directory_const_js_1.BuildArtifactConst.DEFAULT_ROUTER_MAP,
      o = path_1.resolve(this.targetData.getPathInfo().getIntermediatesResProfilePath(), `${s}.json`);
    if (fs_extra_1.ensureFileSync(o), fs_extra_1.writeFileSync(o, JSON.stringify({
      routerMap: e
    })), this.shouldCompileResourceForHarDeduplicate()) {
      const t = path_1.resolve(this.pathInfo.getIntermediatesResDeduplicateProfilePath(), `${s}.json`);
      fs_extra_1.outputJSONSync(t, {
        routerMap: e
      });
    }
    const i = path_1.resolve(this.targetData.getPathInfo().getIntermediatesRes(), common_const_js_1.CommonConst.MODULE_JSON),
      r = json_util_js_1.JsonUtil.getJson5Obj(i);
    Object.assign(r.module, {
      routerMap: `$profile:${s}`
    }), fs_extra_1.writeFileSync(i, JSON.stringify(r));
  }
  processRouterMap() {
    var e;
    const t = this.pathInfo.getIntermediatesRouterMap(null !== (e = this.targetService.getRouterMapJsonFileName(this.moduleModel)) && undefined !== e ? e : build_directory_const_js_1.BuildArtifactConst.TEMP_ROUTER_MAP_FILE_NAME);
    if (!fs_extra_1.existsSync(t)) return;
    const s = json_util_js_1.JsonUtil.getJson5Obj(t).routerMap;
    (null == s ? undefined : s.length) && (s.forEach(e => {
      delete e.moduleNodeDir;
    }), this.writeRouterMap(s));
  }
  processStartupConfig() {
    var e, t, s, o, i;
    const r = json_util_js_1.JsonUtil.getJson5Obj(this.intermediateTempStartupFilePath);
    if (undefined === r) return;
    null === (e = r.startupTasks) || undefined === e || e.forEach(e => {
      delete e.moduleName, delete e.srcEntryAbsolutePath, delete e.startupConfigPath;
    }), null === (t = r.appPreloadHintStartupTasks) || undefined === t || t.forEach(e => {
      delete e.moduleName, delete e.startupConfigPath, delete e.isRemoteByteCodeHar;
    });
    const a = path_1.resolve(this.targetData.getPathInfo().getIntermediatesResProfilePath(), `${null !== (s = this.appStartupFileName) && undefined !== s ? s : "startup_config"}.json`);
    if (fs_extra_1.ensureFileSync(a), fs_extra_1.writeFileSync(a, JSON.stringify(r)), this.shouldCompileResourceForHarDeduplicate()) {
      const e = path_1.resolve(this.pathInfo.getIntermediatesResDeduplicateProfilePath(), `${null !== (o = this.appStartupFileName) && undefined !== o ? o : "startup_config"}.json`);
      fs_extra_1.outputJSONSync(e, r);
    }
    if (undefined === this.appStartupPath) {
      const e = path_1.resolve(this.targetData.getPathInfo().getIntermediatesRes(), "module.json"),
        t = json_util_js_1.JsonUtil.getJson5Obj(e);
      t.module.appStartup = `$profile:${null !== (i = this.appStartupFileName) && undefined !== i ? i : "startup_config"}`, fs_extra_1.writeFileSync(e, JSON.stringify(t));
    }
  }
  checkStartupTaskDependencies() {
    const e = json_util_js_1.JsonUtil.getJson5Obj(this.intermediateTempStartupFilePath);
    if (undefined === e) return;
    const [t, s] = this.getAllStartupTaskNames(e);
    this.validateDependencies(e.startupTasks, t), this.validateDependencies(e.appPreloadHintStartupTasks, s);
  }
  validateDependencies(e, t) {
    e && e.forEach(e => {
      e.dependencies && e.dependencies.length > 0 && e.dependencies.forEach(s => {
        t.includes(s) || this._log._buildError(`The dependency startupTask named ${s} in module ${e.moduleName} is not exist.`)._detail(`Check startupConfig file of module ${e.moduleName} and fix it.`)._file(e.startupConfigPath)._printWarn(this.moduleName);
      });
    });
  }
  getAllStartupTaskNames(e) {
    var t, s;
    const o = [],
      i = [];
    return null === (t = e.startupTasks) || undefined === t || t.forEach(e => {
      o.push(e.name);
    }), null === (s = e.appPreloadHintStartupTasks) || undefined === s || s.forEach(e => {
      i.push(e.name);
    }), [o, i];
  }
  shouldCompileResourceForHarDeduplicate() {
    return !!this.targetService.shouldDoDeduplicateTask() && !!this.moduleModel.isHspModule() && fs_extra_1.existsSync(this.resConfigFilePathForHarDeduplicate);
  }
}
