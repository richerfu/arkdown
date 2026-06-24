/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/package-app.js
 */
import os_1 from "os";
import path_1 from "path";
import process_1 from "process";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as hvigor_config_loader_js_1 from "@ohos/hvigor/src/common/util/hvigor-config-loader";
import * as compressing_1 from "compressing";
import fs_extra_1 from "fs-extra";
import * as packing_tool_options_js_1 from "../builder/inner-java-command-builder/packing-tool-options";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as build_mode_const_js_1 from "../const/build-mode-const";
import * as common_const_js_1 from "../const/common-const";
import * as sdk_const_js_1 from "../const/sdk-const";
import * as module_type_enum_js_1 from "../enum/module-type-enum";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
import * as file_util_js_1 from "../utils/file-util";
import * as inject_util_js_1 from "../utils/inject-util";
import * as json_util_js_1 from "../utils/json-util";
import * as process_utils_js_1 from "../utils/process-utils";
import * as remote_hsp_utils_js_1 from "../utils/remote-hsp-utils";
import * as task_util_js_1 from "../utils/task-util";
import * as base_pack_app_task_js_1 from "./base/base-pack-app-task";
import * as task_names_js_1 from "./common/task-names";
import * as generate_pack_res_js_1 from "./generate-pack-res";
import * as module_task_service_js_1 from "./service/module-task-service";
var __decorate = this && this.__decorate || function (e, t, s, a) {
  var i,
    o = arguments.length,
    l = o < 3 ? t : null === a ? a = Object.getOwnPropertyDescriptor(t, s) : a;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) l = Reflect.decorate(e, t, s, a);else for (var r = e.length - 1; r >= 0; r--) (i = e[r]) && (l = (o < 3 ? i(l) : o > 3 ? i(t, s, l) : i(t, s)) || l);
  return o > 3 && l && Object.defineProperty(t, s, l), l;
};
var Task = task_names_js_1.TaskNames.Task;
export class PackageApp extends base_pack_app_task_js_1.BasePackAppTask {
  get alignDeviceTypes() {
    return this.projectModel.isFaMode() ? [] : inject_util_js_1.InjectUtil.getAlignDeviceTypes();
  }
  declareExecutionCommand() {
    return `${this.getPackAppCommand().toString()}`;
  }
  declareInputs() {
    return new Map().set("allDestDir", this.allDestDir);
  }
  declareInputFiles() {
    const e = new hvigor_1.FileSet().addEntries([this.packInfoPath, ...this.allSrcHapPath, ...this.allSrcHspPath], {
      isDirectory: false
    });
    return this.needPackRes && e.addEntry(this.packResPath, {
      isDirectory: true
    }), fs_extra_1.existsSync(this.pacJsonFilePath) && e.addEntry(this.pacJsonFilePath), e;
  }
  declareOutputFiles() {
    return super.declareOutputFiles().addEntries([path_1.resolve(this.service.getPathInfo().getProjectOutputPath(), this.service.getAppOutputFileName()), ...this.allDestHapPath, ...this.allDestHspPathList], {
      isDirectory: false
    }).addEntry(this.getDestinationPath(), {
      isDirectory: true
    });
  }
  constructor(e, t) {
    super(e, t, Task.PACKAGE_APP), this.allDestDir = [], this.allSrcHapPath = [], this.allDestHapPath = [], this.allSrcHspPath = [], this.allDestHspPathList = [], this.allRemoteHspPathMap = new Map(), this.remoteHspModuleSet = new Set(), this.projectBuildProfilePath = "", this.defaultPackageLimitSize = 2, this.projectBuildProfilePath = t.getProjectModel().getProfilePath(), this.curProduct = t.getTargetProduct(), this.projectModel = t.getProjectModel(), this.pacJsonFilePath = path_1.resolve(this.service.getPathInfo().getProjectOutputPath(), common_const_js_1.CommonConst.PAC_ONLY_JSON), this.normalizedOutputPath = path_1.resolve(this.service.getPathInfo().getProjectOutputPath(), build_directory_const_js_1.BuildDirConst.PACKAGE_APP);
  }
  initTaskDepends() {
    this.dependsOn(generate_pack_res_js_1.GeneratePackRes.name);
  }
  async beforeAlwaysAction() {
    const e = this.service.getProjectModel().getRemoteHspPath();
    this.service.getProductDataMap().forEach(t => {
      for (const s of t) {
        const t = s.getPathInfo().getModuleBuildOutputPath(),
          a = (0, task_util_js_1.isFastBuildApp)(this.service);
        s.getModuleModel().isHapModule() ? a ? this.fastInitWithHapModuleData(s, t) : this.initWithHapModuleData(s, t) : a ? this.fastInitWithHspModuleData(s, t) : this.initWithHspModuleData(s, t);
        const i = s.getModuleModel(),
          o = new dependency_manager_js_1.DependencyManager(this.service.getProjectModel().isFaMode(), i, this.service.getProjectModel()),
          {
            npmDependencies: l
          } = o.collectAllDependencies(),
          r = i.getRemoteHspPath();
        this.remoteHspModuleSet.has(i.getName()) || ((0, remote_hsp_utils_js_1.initSignRemoteHspMap)(r, this.allRemoteHspPathMap, l), (0, remote_hsp_utils_js_1.initSignRemoteHspMap)(e, this.allRemoteHspPathMap, l), this.remoteHspModuleSet.add(i.getName()));
      }
    });
    [...this.allRemoteHspPathMap.values()].forEach(e => {
      const t = this.projectModel.getCacheIntegratedHspPath(this.curProduct.name),
        s = e.isIntegratedHsp ? path_1.resolve(t, e.hspDirName, e.hspFileName) : e.hspPath;
      this.allDestHspPathList.push(s);
    });
  }
  isHapOrHspFile(e) {
    return e.endsWith(build_directory_const_js_1.BuildArtifactExtension.DOT_HSP) || e.endsWith(build_directory_const_js_1.BuildArtifactExtension.DOT_HAP);
  }
  async normalizeDeviceTypes() {
    fs_extra_1.existsSync(this.normalizedOutputPath) && (await fs_extra_1.remove(this.normalizedOutputPath));
    if (!this.alignDeviceTypes.length) return;
    let e = this.allDestHapPath.concat(this.allDestHspPathList);
    if ((0, task_util_js_1.isFastBuildApp)(this.service) && (e = e.filter(e => this.isHapOrHspFile(e))), !e.length) return;
    await fs_extra_1.ensureDir(this.normalizedOutputPath);
    const t = this.getNormalizeDeviceTypesCommand(e);
    this._log._printDebugCommand("NormalizeDeviceTypes", t), await new process_utils_js_1.ProcessUtils().execute(t);
  }
  getUnpackNormalizedPaths(e, t) {
    return e.filter(e => !this.isHapOrHspFile(e) || t.has(path_1.basename(e))).map(e => this.isHapOrHspFile(e) ? path_1.resolve(this.normalizedOutputPath, path_1.basename(e)) : e);
  }
  getUnpackModules() {
    if (!this.alignDeviceTypes.length) return {
      hspList: this.allDestHspPathList,
      hapList: this.allDestHapPath
    };
    const e = json_util_js_1.JsonUtil.getJson5Obj(this.packInfoPath),
      t = new Set();
    return e.packages.forEach(e => {
      t.add(`${e.name}${e.moduleType === module_type_enum_js_1.ModuleType.Shared ? build_directory_const_js_1.BuildArtifactExtension.DOT_HSP : build_directory_const_js_1.BuildArtifactExtension.DOT_HAP}`);
    }), {
      hapList: this.getUnpackNormalizedPaths(this.allDestHapPath, t),
      hspList: this.getUnpackNormalizedPaths(this.allDestHspPathList, t)
    };
  }
  async doTaskAction() {
    const e = "generate app packaging command",
      t = this.durationEvent.createSubEvent(e, "");
    t.start(), await this.getUnpackedHaps(), await this.normalizeDeviceTypes();
    const s = this.getPackAppCommand();
    this._log._printDebugCommand("PackageApp", s), t.stop(), t.setLog(e, hvigor_1.MetricLogType.INFO);
    const a = "execute app packaging command",
      i = this.durationEvent.createSubEvent(a, "");
    if (i.start(), await new process_utils_js_1.ProcessUtils().execute(s), i.stop(), i.setLog(a, hvigor_1.MetricLogType.INFO), this.shouldCollectProjectDebugSymbol()) {
      const e = "collect and pack debug symbol in app scope",
        t = this.durationEvent.createSubEvent(e, "");
      t.start(), this.collectDebugSymbolInAppScope(), await this.buildZipPackage(), t.stop(), t.setLog(e, hvigor_1.MetricLogType.INFO);
    }
  }
  getNormalizeDeviceTypesCommand(e) {
    const t = new packing_tool_options_js_1.PackingToolOptions();
    return t.addCalledJarFile(this.packageTool), t.addMode(sdk_const_js_1.AppPackingToolMode.GeneralNormalize).addDeviceTypes(this.alignDeviceTypes.join(",")).addInputList(e.join(",")).addOutPath(this.normalizedOutputPath), t.build();
  }
  getPackAppCommand() {
    const e = new packing_tool_options_js_1.PackingToolOptions();
    e.addCalledJarFile(this.packageTool), this.needPackRes && fs_extra_1.existsSync(this.packResPath) && e.addPackResPath(this.packResPath);
    const t = (0, task_util_js_1.isFastBuildApp)(this.service) ? "fastApp" : "app",
      {
        hapList: s,
        hspList: a
      } = this.getUnpackModules();
    e.addMode(t).addPackInfoPath(this.packInfoPath).addHapPath(s.join(",")).addHspPath(a.join(",")).force(true).addOutPath(path_1.resolve(this.service.getPathInfo().getProjectOutputPath(), this.service.getAppOutputFileName())).addMainModuleLimit(this.defaultPackageLimitSize).addNormalModuleLimit(this.defaultPackageLimitSize);
    const i = hvigor_config_loader_js_1.HvigorConfigLoader.getInstance().getPropertiesConfigValue(common_const_js_1.HvigorConfigConst.OHOS_PACK_COMPRESS_LEVEL);
    return i && e.addCompressLevel(i, this.service.getSdkInfo().getSdkVersion()), fs_extra_1.existsSync(this.pacJsonFilePath) && e.addPacArg(this.pacJsonFilePath), e.build();
  }
  async getUnpackedHaps() {
    for (const e of this.allDestDir) file_util_js_1.FileUtil.checkDirWithoutDelete(e);
    const e = [],
      t = [];
    this.checkOutputName(this.allSrcHapPath);
    for (let t = 0; t < this.allSrcHapPath.length; t++) e.push(fs_extra_1.copy(this.allSrcHapPath[t], this.allDestHapPath[t]));
    this.checkOutputName(this.allSrcHspPath);
    for (let e = 0; e < this.allSrcHspPath.length; e++) t.push(fs_extra_1.copy(this.allSrcHspPath[e], this.allDestHspPathList[e]));
    await Promise.all([...e, ...t]);
  }
  initWithHapModuleData(e, t) {
    const s = e.getModuleTargetOutputFileName("", false),
      a = path_1.resolve(t, "app");
    this.allDestDir.push(a);
    let i = e.getRelatedEntryModules();
    const o = e.getModuleModel().getModuleType();
    if (module_task_service_js_1.ModuleTaskService.checkEntryModules(o, i)) {
      this.service.isFaMode() || (i = [i[0]]);
      for (const s of i) {
        const i = e.getModuleTargetOutputFileName(s, false),
          o = path_1.resolve(t, i),
          l = this.destFileResolve(a, i);
        this.allSrcHapPath.push(o), this.allDestHapPath.push(l);
      }
    } else if (e.isSingleDeviceTypeTarget()) {
      const e = path_1.resolve(t, s),
        i = this.destFileResolve(a, s);
      this.allDestDir.push(a), this.allSrcHapPath.push(e), this.allDestHapPath.push(i);
    } else e.getTargetDeviceTypeClasses().forEach(s => {
      const i = e.getModuleTargetOutputFileName("", false, undefined, s),
        o = path_1.resolve(t, i),
        l = this.destFileResolve(a, i);
      this.allDestDir.push(a), this.allSrcHapPath.push(o), this.allDestHapPath.push(l);
    });
  }
  isModulePackable(e) {
    const t = this.alignDeviceTypes;
    if (!t.length) return true;
    const s = e.getTargetDeviceType();
    return t.every(e => !!(null == s ? undefined : s.includes(e)));
  }
  fastInitWithHapModuleData(e, t) {
    if (!t || !this.isModulePackable(e)) return;
    const s = path_1.resolve(t, "app"),
      a = e.getModuleModel().getModuleType(),
      i = e.getPathInfo().getFastAppStorageDirectory();
    let o = e.getRelatedEntryModules();
    if (this.allSrcHapPath.push(i), this.allDestDir.push(s), module_task_service_js_1.ModuleTaskService.checkEntryModules(a, o)) {
      this.service.isFaMode() || (o = [o[0]]);
      for (const e of o) {
        const t = this.destFileResolve(s, e);
        this.allDestHapPath.push(t);
      }
    } else {
      const t = this.destFileResolve(s, e.getModuleModel().getName());
      this.allDestHapPath.push(t);
    }
  }
  initWithHspModuleData(e, t) {
    const s = e.getModuleTargetOutputFileName("", false, build_directory_const_js_1.BuildArtifactExtension.DOT_HSP),
      a = path_1.resolve(t, s),
      i = path_1.resolve(t, "app");
    this.allDestDir.push(i);
    const o = path_1.resolve(i, s.replace("-unsigned", ""));
    this.allSrcHspPath.push(a), this.allDestHspPathList.push(o);
  }
  fastInitWithHspModuleData(e, t) {
    if (!t || !this.isModulePackable(e)) return;
    const s = path_1.resolve(t, "app"),
      a = e.getPathInfo().getFastAppStorageDirectory(),
      i = this.destFileResolve(s, e.getModuleModel().getName());
    this.allDestDir.push(s), this.allSrcHspPath.push(a), this.allDestHspPathList.push(i);
  }
  checkOutputName(e) {
    if ((0, task_util_js_1.isFastBuildApp)(this.service)) return;
    const t = new Set();
    e.forEach(e => {
      const s = e.slice(e.lastIndexOf(path_1.sep) + 1);
      t.has(s) ? this._log.printErrorExit("DUPLICATE_OUTPUT_NAME", [s, e]) : t.add(s);
    });
  }
  shouldCollectProjectDebugSymbol() {
    var e, t;
    return null !== (e = hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.USE_COLLECT_DEBUG_SYMBOL)) && undefined !== e ? e : !(null === (t = this.service.getBuildOption()) || undefined === t ? undefined : t.debuggable);
  }
  collectDebugSymbolInAppScope() {
    const e = this.getDestinationPath();
    fs_extra_1.ensureDirSync(e), this.service.getProductDataMap().forEach(t => {
      for (const s of t) {
        const t = s.getPathInfo(),
          a = s.getModuleModel().getName(),
          i = t.getModuleBuildOutputPath(),
          o = this.getSrcModuleDebugSymbolPath(i);
        this.createSymbolicLink(o, e, a);
      }
    });
  }
  getDestinationPath() {
    var e;
    return (null === (e = this.service.getBuildOption()) || undefined === e ? undefined : e.debuggable) ? path_1.resolve(this.pathInfo.getProjectDebugSymbolPath(), build_mode_const_js_1.BuildModeConst.DEBUG) : path_1.resolve(this.pathInfo.getProjectDebugSymbolPath(), build_mode_const_js_1.BuildModeConst.RELEASE);
  }
  getSrcModuleDebugSymbolPath(e) {
    var t;
    if (!e) return "";
    const s = (null === (t = this.service.getBuildOption()) || undefined === t ? undefined : t.debuggable) ? path_1.resolve(e, build_directory_const_js_1.BuildDirConst.OUTPUT_SYMBOL, build_mode_const_js_1.BuildModeConst.DEBUG) : path_1.resolve(e, build_directory_const_js_1.BuildDirConst.OUTPUT_SYMBOL, build_mode_const_js_1.BuildModeConst.RELEASE);
    return fs_extra_1.existsSync(s) || fs_extra_1.mkdirSync(s, {
      recursive: true
    }), s;
  }
  createSymbolicLink(e, t, s) {
    if (!e || !t || !s) return;
    if (!fs_extra_1.existsSync(e)) return;
    const a = "win32" === process_1.platform || "Windows_NT" === os_1.type() ? "junction" : "dir",
      i = path_1.resolve(t, s);
    try {
      !fs_extra_1.existsSync(i) && fs_extra_1.existsSync(t) && fs_extra_1.symlinkSync(e, i, a);
    } catch (t) {
      this._log.warn(`Create symbolic link for ${e} failed. Reason: ${t}`);
    }
  }
  async buildZipPackage() {
    const e = this.getDestinationPath(),
      t = path_1.resolve(e, "..", "temp");
    fs_extra_1.mkdirSync(t, {
      recursive: true
    });
    const s = path_1.resolve(t, `${build_directory_const_js_1.BuildArtifactConst.APP_SYMBOL}${build_directory_const_js_1.BuildArtifactExtension.DOT_ZIP}`),
      a = path_1.resolve(e, `${build_directory_const_js_1.BuildArtifactConst.APP_SYMBOL}${build_directory_const_js_1.BuildArtifactExtension.DOT_ZIP}`);
    try {
      fs_extra_1.existsSync(a) && fs_extra_1.unlinkSync(a), this._log.debug("Remove the invalid app-symbol.zip file successfully.");
      if (await compressing_1.zip.compressDir(e, s, {
        ignoreBase: true
      }).then(() => true).catch(() => false)) return void fs_extra_1.renameSync(s, a);
      this._log.debug("Packing app-symbol.zip failed using 'compressing'."), fs_extra_1.removeSync(s);
      const t = (0, hvigor_common_1.getZipWriter)("application/zip"),
        i = new Set();
      await (0, hvigor_common_1.addFilesToZip)(t, e, i);
      const o = await t.close(),
        l = Buffer.from(await o.arrayBuffer());
      fs_extra_1.writeFileSync(s, l), fs_extra_1.renameSync(s, a);
    } catch (e) {
      this._log.warn(`Packing app-symbol.zip failed. Reason: ${e}`);
    } finally {
      fs_extra_1.removeSync(t);
    }
  }
  destFileResolve(e, t) {
    return path_1.resolve(e, t.replace("-unsigned", ""));
  }
}
__decorate([(0, hvigor_1.Input)()], PackageApp.prototype, "alignDeviceTypes", null);
