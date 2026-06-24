/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-libs.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as fs from "fs-extra";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as module_type_enum_js_1 from "../enum/module-type-enum";
import * as collect_duplicate_har_dependencies_js_1 from "../har-dedepulicate/collect_duplicate_har_dependencies";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
import * as cmake_util_js_1 from "../utils/cmake/cmake-util";
import * as file_util_js_1 from "../utils/file-util";
import * as har_target_util_js_1 from "../utils/har-target-util";
import * as hsp_target_utils_js_1 from "../utils/hsp-target-utils";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as build_native_with_ninja_js_1 from "./build-native-with-ninja";
import * as process_libs_common_properties_js_1 from "./common-properties/process-libs-common-properties";
import * as task_names_js_1 from "./common/task-names";
import * as process_libs_js_1 from "./worker/process-libs";
var Task = task_names_js_1.TaskNames.Task;
export class ProcessLibs extends process_libs_common_properties_js_1.ProcessLibsCommonProperties {
  taskShouldDo() {
    return this.moduleModel.isAtomicService() && fs.ensureDirSync(this.pathInfo.getIntermediatesProcessLibs()), !this.moduleModel.isAtomicService();
  }
  async beforeAlwaysAction() {
    this.init(), this.service.isFaMode() && (this.allHarLibs = this.getAllHarLibs(), this.libsWithPkg = this.getHarLibsWithPkg()), this.hasLocalNativeOutput && this.localOutput && (this.localOutputHash = await file_util_js_1.FileUtil.hashEntry(this.localOutput));
  }
  declareInputs() {
    var e;
    const t = super.declareInputs();
    return undefined !== this.filterRule && Object.entries(this.filterRule).filter(([, e]) => !!e).forEach(([e, i]) => t.set(e, i)), this.hasLocalNativeOutput && this.localOutputHash && this.localOutputHash.forEach((e, i) => {
      this.localOutput && t.set(path_1.relative(this.localOutput, i), e);
    }), t.set("isBundledDependencies", this.isBundledDependencies).set("isByteCodeHar", this.targetService.isByteCodeHar()).set("excludeFromHar", null === (e = this.targetService.getNativeLibOption().excludeFromHar) || undefined === e || e), t;
  }
  declareInputFiles() {
    const e = new hvigor_1.FileSet(),
      t = path_1.resolve(this.moduleModel.getProjectDir(), build_directory_const_js_1.BuildDirConst.LIBS);
    fs.existsSync(t) && e.addEntry(t, {
      isDirectory: true
    });
    for (const t of this.allHarLibs.keys()) fs.existsSync(t) && e.addEntry(t);
    return fs.existsSync(this.projectModel.getProfilePath()) && e.addEntry(this.projectModel.getProfilePath()), fs.existsSync(this.moduleModel.getProfilePath()) && e.addEntry(this.moduleModel.getProfilePath()), this.cangjieLibs && fs.existsSync(this.cangjieLibs) && e.addEntry(this.cangjieLibs, {
      isDirectory: true
    }), this.targetService.shouldDoDeduplicateTask() && this.moduleModel.isHapModule() && this.processHarDeduplicateInputFiles(e), e;
  }
  declareOutputFiles() {
    if (!this.libsOutputDir) return new hvigor_1.FileSet();
    const e = new hvigor_1.FileSet();
    return e.addEntry(this.libsOutputDir, {
      isDirectory: true
    }), this.pkgsDuplicate && this.pkgsDuplicate.length > 0 && e.addEntry(this.pathInfo.getSoDuplicateInfoPath()), e;
  }
  constructor(e) {
    super(e, Task.PROCESS_LIB), this.logger = ohos_logger_js_1.OhosLogger.getLogger(ProcessLibs.name), this.allHarLibs = new Map(), this.libsWithPkg = [];
  }
  init() {
    var e, t, i;
    const s = this.targetService,
      r = s.getTargetData().getPathInfo(),
      a = cmake_util_js_1.CmakeUtil.checkAbiFilters(null === (e = s.getBuildOption().externalNativeOptions) || undefined === e ? undefined : e.abiFilters, s.getTargetData().isHarmonyOS(), this.moduleModel, this.targetName);
    this.collectAllLibs = !!(null === (t = s.getBuildOption().nativeLib) || undefined === t ? undefined : t.collectAllLibs), this.allHarLibs = this.getAllHarLibs(), this.libsWithPkg = this.getHarLibsWithPkg(), this.libsOutputDir = r.getIntermediatesProcessLibs(), this.localOutput = r.getIntermediatesCppOutPut(), this._nativeOption = s.getBuildOption().externalNativeOptions, this.hasLocalNativeOutput = cmake_util_js_1.CmakeUtil.nativeTaskCondition(this.moduleModel, this.targetData, this._nativeOption), this.filterRule = s.getNativeLibOption().filter, this.readElf = s.getSdkInfo().getSdkLlvmReadElf(), this.compileCommandsPath = a.map(e => path_1.resolve(r.getNinjaWorkDir(), e, common_const_js_1.CommonConst.COMPILE_COMMANDS_JSON)), this.hasLocalNativeOutput && (this.nativeLibs = a.map(e => cmake_util_js_1.CmakeUtil.parseLibraries(path_1.resolve(r.getNinjaWorkDir(), e), this.targetName, e))), this.cangjieLibs = path_1.resolve(r.getIntermediatesCangjieOutPut()), this.isHarType = this.moduleModel.getModuleType() === module_type_enum_js_1.ModuleType.Har, this.nativeOptionArg = null === (i = this._nativeOption) || undefined === i ? undefined : i.arguments;
  }
  get isBundledDependencies() {
    return this.targetService.isBundledDependencies();
  }
  initTaskDepends() {
    this.declareDepends(build_native_with_ninja_js_1.BuildNativeWithNinja.name);
  }
  async doTaskAction() {
    const e = this.service.getDependencyInfo().getSelfAsDependency(),
      t = path_1.resolve(this.moduleModel.getProjectDir(), "libs"),
      i = this.pathInfo.getIntermediatesCppOutPut();
    if (!this.libsOutputDir || !this.cangjieLibs) return void this.logger.warn("processLibs task error: this.libsOutputDir or this.cangjieLibs is undefined");
    const s = {
        allHarLibs: this.libsWithPkg,
        outputDir: this.libsOutputDir,
        filterRule: this.filterRule,
        localLibs: (0, process_libs_js_1.buildLibs)(this.getLibsInfo(t), e, t),
        outputLibs: (0, process_libs_js_1.buildLibs)(this.getLibsInfo(i), e, i),
        profilePath: cmake_util_js_1.CmakeUtil.getMayBeOccurErrorsFilePath(this.moduleModel),
        moduleName: this.moduleName,
        cangjieLibs: (0, process_libs_js_1.buildLibs)(this.getLibsInfo(this.cangjieLibs), e, this.cangjieLibs),
        collectAllLibs: this.collectAllLibs,
        readElf: this.readElf,
        localOutputDir: this.localOutput,
        compileCommandsPath: this.compileCommandsPath,
        isHarType: this.isHarType,
        nativeOptionArg: this.nativeOptionArg,
        nativeLibs: this.nativeLibs
      },
      r = "process libs",
      a = this.durationEvent.createSubEvent(r, "");
    a.start(), this.logger.debug("Worker dispatch failed, running processLibs on main thread."), await (0, process_libs_js_1.processLibs)(s, this.logger), a.stop(), a.setLog(r, hvigor_1.MetricLogType.INFO), this.targetService.shouldDoDeduplicateTask() && this.moduleModel.isHspModule() && this.writeDuplicatePkgsToIntermediateFile();
  }
  getHarLibsWithPkg() {
    const e = [];
    for (const [t, i] of this.allHarLibs.entries()) e.push(...(0, process_libs_js_1.buildLibs)(this.getLibsInfo(t), i, t));
    return e;
  }
  getAllHarLibs() {
    const e = new Map();
    if (this.collectRemoteHarLibs(this.targetService, e), this.moduleModel.isHarModule() && this.targetService.getTargetData().getTargetName() !== common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET) return this.isBundledDependencies ? (this.service.getHarModuleDependencies().forEach(t => {
      const i = har_target_util_js_1.HarTargetUtil.getHarTargetData(t, this.moduleModel, this.targetService.getTargetData().getTargetName(), this.moduleName, this.targetService),
        s = dependency_manager_js_1.DependencyManager.getModuleModelByDependency(t[1], this.projectModel);
      if (s) {
        const t = this.projectModel.getTarget(null == s ? undefined : s.getName(), i);
        this.collectLocalHarLibs(t, e);
      }
    }), e) : e;
    const t = har_target_util_js_1.HarTargetUtil.calDepHarTargets(this.targetService);
    for (const [i, s] of t) {
      const t = this.projectModel.getTarget(i, s);
      this.collectLocalHarLibs(t, e);
    }
    return this.targetService.shouldDoDeduplicateTask() && this.collectDuplicateHar(e), e;
  }
  collectLocalHarLibs(e, t) {
    if (e) {
      const i = e.getTargetData().getPathInfo().getIntermediatesProcessLibs(),
        s = e.getModuleService().getDependencyInfo().getSelfAsDependency();
      t.set(i, s), t.set(s.getDefaultLibsDir(), s);
    }
  }
  collectRemoteHarLibs(e, t) {
    this.service.getRemoteHarDependencies().forEach(e => {
      if (this.isBundledDependencies) {
        if (e.isByteCodeHarDependency()) return;
        t.set(e.getDefaultLibsDir(), e);
      } else {
        if (this.moduleModel.isHarModule() && this.targetService.getTargetData().getTargetName() !== common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET) return;
        t.set(e.getDefaultLibsDir(), e);
      }
    });
  }
  collectDuplicateHar(e) {
    this.moduleModel.isHapModule() ? this.collectDuplicateHarForHap(e) : this.moduleModel.isHspModule() && this.excludeDuplicateHarForHsp(e);
  }
  collectDuplicateHarForHap(e) {
    const t = (0, collect_duplicate_har_dependencies_js_1.getMergedHapAllHspDependencies)();
    hsp_target_utils_js_1.HspTargetUtils.calHspTargets(this.targetService, t).forEach((t, i) => {
      if (!(0, collect_duplicate_har_dependencies_js_1.getHspNeedDeduplicateHarInfoMap)(i)) return;
      const s = t.getPathInfo().getSoDuplicateInfoPath();
      if (!fs.existsSync(s)) return;
      const r = this.projectModel.getTarget(i, t.getTargetName());
      if (!r) return void this.logger.debug(`Can not find ${i} target ${t.getTargetName()}`);
      const a = fs.readJSONSync(s),
        o = r.getModuleService().getAllDependencies(),
        l = new Map();
      o.forEach(e => {
        l.set(e.getDependencyRootPath(), e);
      }), a.forEach(t => {
        if (e.has(t.libsPath)) return;
        const i = l.get(t.dependencyRootPath);
        i ? (e.set(t.libsPath, i), this.logger.debug(`Add so libPath ${t.libsPath}`)) : this.logger.debug(`Can not find dependency ${t.dependencyRootPath}`);
      });
    });
  }
  excludeDuplicateHarForHsp(e) {
    var t;
    const i = null !== (t = (0, collect_duplicate_har_dependencies_js_1.getHspNeedDeduplicateHarInfoMap)(this.moduleName)) && undefined !== t ? t : {},
      s = new Set(Object.keys(i)),
      r = [],
      a = new Set();
    for (const [t, i] of e.entries()) s.has(i.getDependencyRootPath()) && (a.add(t), this.logger.debug(`Exclude har ${i.getPackageName()} from ${this.moduleName}`), r.push({
      libsPath: t,
      dependencyRootPath: i.getDependencyRootPath()
    }));
    this.pkgsDuplicate = r;
    for (const t of a) e.delete(t);
  }
  writeDuplicatePkgsToIntermediateFile() {
    this.pkgsDuplicate && this.pkgsDuplicate.length > 0 && fs.outputJSONSync(this.pathInfo.getSoDuplicateInfoPath(), this.pkgsDuplicate);
  }
  processHarDeduplicateInputFiles(e) {
    const t = (0, collect_duplicate_har_dependencies_js_1.getMergedHapAllHspDependencies)();
    hsp_target_utils_js_1.HspTargetUtils.calHspTargets(this.targetService, t).forEach((t, i) => {
      const s = t.getPathInfo().getSoDuplicateInfoPath();
      fs.existsSync(s) && e.addEntry(s);
    });
  }
}
