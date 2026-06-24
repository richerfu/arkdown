/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-resource.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
import fs_extra_1 from "fs-extra";
import * as opt_compression_builder_js_1 from "../builder/opt-compression-builder";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as collect_duplicate_har_dependencies_js_1 from "../har-dedepulicate/collect_duplicate_har_dependencies";
import * as hsp_target_utils_js_1 from "../utils/hsp-target-utils";
import * as inject_util_js_1 from "../utils/inject-util";
import * as abstract_process_resource_js_1 from "./abstract/abstract-process-resource";
import * as task_names_js_1 from "./common/task-names";
import * as unit_test_process_profile_js_1 from "./unitTest/unit-test-process-profile";
var __decorate = this && this.__decorate || function (e, t, s, o) {
  var i,
    r = arguments.length,
    a = r < 3 ? t : null === o ? o = Object.getOwnPropertyDescriptor(t, s) : o;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) a = Reflect.decorate(e, t, s, o);else for (var l = e.length - 1; l >= 0; l--) (i = e[l]) && (a = (r < 3 ? i(a) : r > 3 ? i(t, s, a) : i(t, s)) || a);
  return r > 3 && a && Object.defineProperty(t, s, a), a;
};
var Task = task_names_js_1.TaskNames.Task;
const ohosTestTargetName = common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET;
export class ProcessResource extends abstract_process_resource_js_1.AbstractProcessResource {
  constructor(e) {
    if (super(e, Task.PROCESS_RESOURCE), this._log = hvigor_1.HvigorLogger.getLogger("process-resource"), this.resConfigFilePath = path_1.resolve(this.outputDir, common_const_js_1.CommonConst.RES_CONFIG_FILE), this.optCompressionFilePath = path_1.resolve(this.outputDir, common_const_js_1.CommonConst.OPT_COMPRESSION_FILE), this.optCompressionBuilder = this.getOptCompressionBuilder(), this.targetService.shouldDoDeduplicateTask() && this.moduleModel.isHapModule()) {
      const e = (0, collect_duplicate_har_dependencies_js_1.getMergedHapAllHspDependencies)();
      this.allHspDepTargets = hsp_target_utils_js_1.HspTargetUtils.calHspTargets(this.targetService, e);
    }
  }
  initTaskDepends() {
    super.initTaskDepends(), inject_util_js_1.InjectUtil.isUnitTestProcess() && this.declareDepends(unit_test_process_profile_js_1.UnitTestProcessProfile.name);
  }
  beforeTask() {
    super.beforeTask(), fs_extra_1.existsSync(this.optCompressionFilePath) && fs_extra_1.removeSync(this.optCompressionFilePath);
  }
  getRestoolConfigBuilder() {
    const e = super.getRestoolConfigBuilder();
    return this.processHarResourceDeduplicate(e), e;
  }
  declareInputs() {
    const e = super.declareInputs();
    return e.set(this.optCompressionFilePath, JSON.stringify(this.optCompressionBuilder.getOptCompressionObj())), e.set("resource_str", fs_extra_1.existsSync(path_1.resolve(this.pathInfo.getIntermediatesRes(), "resource_str"))), e.set("isPreviewMode", inject_util_js_1.InjectUtil.isPreviewProcess()), e;
  }
  get ignoreResourcePattern() {
    var e;
    return null === (e = this.targetService.getBuildOption().resOptions) || undefined === e ? undefined : e.ignoreResourcePattern;
  }
  get qualifiersConfig() {
    var e;
    return null === (e = this.targetService.getBuildOption().resOptions) || undefined === e ? undefined : e.qualifiersConfig;
  }
  doTaskAction() {
    this.restoolConfigBuilder.addInputDir(this.pathInfo.getResourceStr(), build_directory_const_js_1.ResToolInputTypeConst.HAR);
    const e = this.optCompressionBuilder.getOptCompressionObj();
    fs_extra_1.outputJSONSync(this.optCompressionFilePath, e), this.restoolConfigBuilder.addCompression(this.optCompressionFilePath), this.restoolConfigBuilder.setResCompileThread(this.processResCompileThreads()), this.restoolConfigBuilder.setIgnoreResourcePattern(this.processIgnoreResourcePattern(this.ignoreResourcePattern)), this.restoolConfigBuilder.setQualifiersConfig(this.service.getModuleModel().isHarModule(), this.qualifiersConfig), super.doTaskAction(), this.targetService.shouldDoDeduplicateTask() && this.resourcesDuplicate && this.resourcesDuplicate.length > 0 && (this.writeDuplicateResourcesToIntermediateFile(), this.writeHspDeduplicateResConfig());
  }
  declareInputFiles() {
    const e = super.declareInputFiles();
    if (this.shouldDoHarDeduplicate()) {
      const t = this.pathInfo.getIntermediatesIdDefinedJsonFile();
      fs_extra_1.existsSync(t) && e.addEntry(t), this.moduleModel.isHapModule() && this.processHarDeduplicateInputFiles(e);
    }
    return e;
  }
  declareOutputFiles() {
    const e = super.declareOutputFiles();
    return e.addEntry(this.optCompressionFilePath, {
      isDirectory: false
    }), this.resourcesDuplicate && this.resourcesDuplicate.length > 0 && (e.addEntry(this.pathInfo.getResourcesDuplicateInfoPath()), e.addEntry(this.pathInfo.getDeduplicateResConfigPath())), e;
  }
  getAppResourceDir() {
    var e;
    const t = this.targetData.getProduct();
    return undefined !== t.resource && 0 !== t.resource.directories.length ? path_1.resolve(this.projectModel.getProjectDir(), null === (e = t.resource) || undefined === e ? undefined : e.directories[0]) : this.projectModel.getAppRes().getResourcePath();
  }
  get includeAppScopeRes() {
    var e;
    return null === (e = this.targetService.getBuildOption().resOptions) || undefined === e ? undefined : e.includeAppScopeRes;
  }
  initCommandBuilder() {
    (this.service.getModuleModel().isHapModule() || this.service.getModuleModel().isHspModule() && false !== this.includeAppScopeRes || this.targetName === ohosTestTargetName) && this.restoolConfigBuilder.addInputDir(this.getAppResourceDir(), "App");
    const e = this.projectModel.getAppRes().getAppResOpt().app.bundleName,
      t = path_1.resolve(this.pathInfo.getIntermediatesRes(), "ids_map");
    if (this.restoolConfigBuilder.addJsonFile(this.processedJson).addModulePackName(e).addOutputDir(this.outputDir).addResTable(path_1.resolve(this.pathInfo.getGenerateSourceR(), build_directory_const_js_1.BuildArtifactConst.RESOURCE_TABLE_H)).addIdsPath(t).addDefinedIds(path_1.resolve(t, "id_defined.json")), this.service.getModuleModel().isHarModule() || inject_util_js_1.InjectUtil.isPreviewProcess() || this.restoolConfigBuilder.addResTable(this.pathInfo.getResourceTableTs()), this.shouldDoHarDeduplicate() && this.processCustomIdDefinedFile(), !this.sdkInfo.isOhos) {
      const e = path_1.resolve(this.sdkInfo.getHosToolchainsDir(), "id_defined.json");
      fs_1.existsSync(e) && this.restoolConfigBuilder.addDefinedSysIds(e);
    }
    this.isEnabledIconCheck() && this.restoolConfigBuilder.setIconCheck(true), this.targetData.getTargetName() === ohosTestTargetName && (this.addOhosTestExtraCompilationResources(this.restoolConfigBuilder), this.addIntermediatesSrcResource(this.restoolConfigBuilder, this.moduleModel.getSourceRootByTargetName(ohosTestTargetName)));
  }
  getOptCompressionBuilder() {
    var e, t;
    const s = null === (e = this.targetService.getBuildOption().resOptions) || undefined === e ? undefined : e.compression,
      o = this.targetData.isHarmonyOS() && ((null === (t = null == s ? undefined : s.media) || undefined === t ? undefined : t.enable) || Array.isArray(null == s ? undefined : s.filters) && s.filters.length > 0) && (this.service.getModuleModel().isHapModule() || this.service.getModuleModel().isHspModule()),
      i = new opt_compression_builder_js_1.OptCompressionBuilder();
    if (i.setExtensionPath(this.sdkInfo.getLibimageTranscoderShared()), o) {
      const e = (0, hvigor_common_1.getOsLanguage)(),
        t = (0, hvigor_logger_1.getUrl)(hvigor_logger_1.DEVELOPER_URL),
        o = "You have enabled the resource compression function, which will change the resource file name and data format. In some cases, you may experience problems with abnormal image displaying, in particular the disappearance of the layered launcher icon.\nYou can set the filter parameters to exclude these files, or to include the files you just need.\n";
      if (t) {
        const s = `${t}/consumer/${e}/doc/harmonyos-guides/ide-hvigor-build-profile-app`;
        this._log.warn(`${o}For details, see compression option in ${s}`);
      } else this._log.warn(`${o}For details, go to the official website to see compression option in Guides > DevEco Studio > Building Your App/Atomic Service > Overview > Configuration Files > build-profile.json5`);
      const r = [...this.restoolConfigBuilder.getResourceDir(), this.getAppResourceDir()];
      i.setResourceDirArr(r).setCompressionConfig(s);
    }
    return i;
  }
  processCustomIdDefinedFile() {
    const e = this.pathInfo.getIntermediatesIdDefinedJsonFile();
    fs_extra_1.existsSync(e) && (this._log.debug(`use custom id defined file ${e}`), this.restoolConfigBuilder.addDefinedIds(e));
  }
  shouldDoHarDeduplicate() {
    return this.targetService.shouldDoDeduplicateTask() && (this.moduleModel.isHapModule() || this.moduleModel.isHspModule());
  }
  writeHspDeduplicateResConfig() {
    const e = (0, wdk_1.cloneDeep)(this.restoolConfigBuilder.getConfigObject());
    e.output = this.pathInfo.getIntermediatesResDeduplicate();
    const t = path_1.resolve(this.pathInfo.getIntermediatesResDeduplicate(), "ids_map");
    fs_extra_1.ensureDirSync(t), e.ids = t, e.ResourceTable = [path_1.resolve(this.pathInfo.getIntermediatesResDeduplicate(), build_directory_const_js_1.BuildArtifactConst.RESOURCE_TABLE_H)];
    const s = new Set(this.resourcesDuplicate);
    e.dependencies = e.dependencies.filter(e => !s.has(e));
    const o = this.pathInfo.getDeduplicateResConfigPath();
    fs_extra_1.outputJSONSync(o, e);
  }
  processHarDeduplicateInputFiles(e) {
    this.allHspDepTargets && this.allHspDepTargets.forEach(t => {
      const s = t.getPathInfo().getResourcesDuplicateInfoPath();
      fs_extra_1.existsSync(s) && e.addEntry(s);
    });
  }
  processHarResourceDeduplicate(e) {
    if (this.targetService.shouldDoDeduplicateTask()) if (this.moduleModel.isHspModule()) {
      const t = e.getConfigObject().dependencies;
      this.collectDuplicateHarResources(t);
    } else this.moduleModel.isHapModule() && this.addDuplicateHarResources(e);
  }
  collectDuplicateHarResources(e) {
    const t = (0, collect_duplicate_har_dependencies_js_1.getHspNeedDeduplicateHarInfoMap)(this.moduleName);
    if (!t) return;
    const s = Object.keys(t).map(e => path_1.normalize(e)),
      o = [];
    e.forEach(e => {
      s.find(t => path_1.normalize(e).startsWith(t)) && o.push(e);
    }), this.resourcesDuplicate = o;
  }
  writeDuplicateResourcesToIntermediateFile() {
    fs_extra_1.outputJSONSync(this.pathInfo.getResourcesDuplicateInfoPath(), this.resourcesDuplicate);
  }
  addDuplicateHarResources(e) {
    this.allHspDepTargets && this.allHspDepTargets.forEach(t => {
      const s = t.getPathInfo().getResourcesDuplicateInfoPath();
      if (!fs_extra_1.existsSync(s)) return;
      const o = fs_extra_1.readJSONSync(s);
      o && o.forEach(t => {
        e.addInputDir(t, build_directory_const_js_1.ResToolInputTypeConst.HAR);
      });
    });
  }
}
__decorate([(0, hvigor_1.Input)()], ProcessResource.prototype, "ignoreResourcePattern", null), __decorate([(0, hvigor_1.Input)()], ProcessResource.prototype, "qualifiersConfig", null), __decorate([(0, hvigor_1.Input)()], ProcessResource.prototype, "includeAppScopeRes", null);
