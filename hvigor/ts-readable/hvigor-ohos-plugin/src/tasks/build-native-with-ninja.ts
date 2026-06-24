/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/build-native-with-ninja.js
 */
import promises_1 from "fs/promises";
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as command_builder_modifier_js_1 from "../builder/command-builder-modifier";
import * as native_command_builder_js_1 from "../builder/native-command-builder";
import * as common_const_js_1 from "../const/common-const";
import * as cpu_abi_enum_js_1 from "../enum/cpu-abi-enum";
import * as collect_duplicate_har_dependencies_js_1 from "../har-dedepulicate/collect_duplicate_har_dependencies";
import * as cmake_util_js_1 from "../utils/cmake/cmake-util";
import * as file_util_js_1 from "../utils/file-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as abstract_build_native_js_1 from "./abstract-build-native";
import * as build_native_with_cmake_js_1 from "./build-native-with-cmake";
import * as task_names_js_1 from "./common/task-names";
var Task = task_names_js_1.TaskNames.Task;
export class BuildNativeWithNinja extends abstract_build_native_js_1.AbstractBuildNative {
  constructor(e) {
    super(e, Task.BUILD_NATIVE_WITH_NINJA), this._log = ohos_logger_js_1.OhosLogger.getLogger(BuildNativeWithNinja.name);
  }
  initTaskDepends() {
    var e, t, i;
    this.declareDepends(build_native_with_cmake_js_1.BuildNativeWithCmake.name);
    for (const a of this.service.getDependencyInfo().getModuleDependencyMap().keys()) {
      const s = null !== (t = null !== (e = this.projectModel.getTarget(a, this.targetName)) && undefined !== e ? e : this.projectModel.getTarget(a, "default")) && undefined !== t ? t : this.projectModel.getTarget(a),
        r = null !== (i = null == s ? undefined : s.getTargetData().getTargetName()) && undefined !== i ? i : "default";
      this.declareDepends(`${r}@${Task.DO_NATIVE_STRIP.name}`, a);
    }
  }
  async buildCommand(e, t, i, a, s) {
    var r;
    const o = t.getPathInfo(),
      n = path_1.resolve(o.getNinjaWorkDir(), e),
      l = new native_command_builder_js_1.NativeCommandBuilder(this.sdkInfo.getNativeNinjaTool()).changeToDir(n),
      u = new command_builder_modifier_js_1.CommandBuilderModifier(l, command_builder_modifier_js_1.CommandBuilderType.NINJA).wrapCommandBuilder().build();
    (null === (r = this._nativeOption) || undefined === r ? undefined : r.targets) && this._nativeOption.targets.length > 0 && u.push(...this._nativeOption.targets), this._log._printDebugCommand("Ninja", u), s.stop(), s.setLog(s.getName(), hvigor_1.MetricLogType.INFO);
    const d = `submit CMake task to work pool with ABI of ${e}`,
      _ = this.durationEvent.createSubEvent(d, "");
    _.start(), await this.executeCommand(u, n, i, a, _), _.stop(), _.setLog(d, hvigor_1.MetricLogType.INFO);
  }
  taskShouldDo() {
    const e = cmake_util_js_1.CmakeUtil.nativeTaskCondition(this.moduleModel, this.targetData, this._nativeOption);
    return e || fs_extra_1.emptyDirSync(this.pathInfo.getIntermediatesCppOutPut()), e;
  }
  async doTaskAction() {
    var e;
    const t = this.targetData.getPathInfo(),
      i = path_1.resolve(t.getIntermediatesCppOutPut()),
      a = cmake_util_js_1.CmakeUtil.checkAbiFilters(null === (e = this._nativeOption) || undefined === e ? undefined : e.abiFilters, this.targetData.isHarmonyOS(), this.moduleModel, this.targetName);
    await this.cleanUnwanted(a), await this.checkAndHandleBuildState(t, a);
    for (const e of a) {
      const a = `generate Ninja command with ABI of ${e}`,
        s = this.durationEvent.createSubEvent(a, "");
      s.start();
      const r = path_1.resolve(i, e),
        o = path_1.resolve(t.getNinjaWorkDir(), e);
      await this.buildCommand(e, this.targetData, async () => {
        const t = cmake_util_js_1.CmakeUtil.parseLibraries(o, this.targetName, e);
        await this.syncOutput(t, r, o);
      }, [], s), await this.syncStl(e, r, o, t);
    }
    await this.cacheBuildInfo(t);
  }
  async cleanUnwanted(e) {
    const t = this.pathInfo.getIntermediatesCppOutPut();
    await this.removeDirectoriesByFilter(t, t => !e.includes(t));
  }
  async removeDirectoriesByFilter(e, t) {
    if (!fs_extra_1.existsSync(e)) return;
    const i = await promises_1.readdir(e, {
      withFileTypes: true
    });
    for (const a of i) a.isDirectory() && t(a.name) && fs_extra_1.rmdirSync(path_1.resolve(e, a.name), {
      recursive: true
    });
  }
  async syncOutput(e, t, i) {
    var a;
    const s = new Map(),
      r = null === (a = this._nativeOption) || undefined === a ? undefined : a.targets;
    for (const [a, o] of e.entries()) r && r.length > 0 && -1 === r.indexOf(a) ? this._log.debug(`Case targets are configured, dependency ${a} are not synchronized.`) : await this.syncLibOutputs(o, t, i, s);
  }
  async syncLibOutputs(e, t, i, a) {
    const s = e.getNameOnDisk();
    if (!s) return;
    const r = path_1.resolve(t, s),
      o = e.getOutputs().map(e => path_1.isAbsolute(e) ? path_1.resolve(e) : path_1.resolve(i, e)).filter(e => fs_extra_1.existsSync(e) && !hvigor_1.PathUtil.checkCopyPathIsSame(e, r));
    for (const e of o) this._log.debug(`External build set its own library output location for ${s}, which will be hard linked or copy to the expected location.`), await file_util_js_1.FileUtil.linkFile(e, r, true);
    await this.syncLibDeps(e, t, i, a);
  }
  async syncLibDeps(e, t, i, a) {
    var s;
    const r = path_1.normalize(this.sdkInfo.getSdkNativeDir()),
      o = this.targetData.isHarmonyOS() ? path_1.normalize(this.sdkInfo.getHmsNativeDir()) : undefined,
      n = null !== (s = (0, collect_duplicate_har_dependencies_js_1.getHspNeedDeduplicateHarInfoMap)(this.moduleName)) && undefined !== s ? s : {},
      l = Object.keys(n),
      u = this.targetService.shouldDoDeduplicateTask();
    for (let s of e.getRuntimeFiles()) {
      s = path_1.isAbsolute(s) ? path_1.resolve(s) : path_1.resolve(i, s), s = path_1.normalize(s);
      const e = l.find(e => s.startsWith(e));
      if (u && this.moduleModel.isHspModule() && e) {
        this._log.debug(`Deduplicate ${s} from har in module ${this.moduleName}.`);
        continue;
      }
      this._log.debug(`Sync native runtime file ${s}.`);
      const n = path_1.resolve(t, path_1.basename(s));
      !fs_extra_1.existsSync(s) || hvigor_1.PathUtil.checkCopyPathIsSame(s, n) || this.excludeFromHsp(s) ? await this.removeHarOutPutSo(s, n) : this.isExcludeFromHar(s) ? this._log.debug(`Exclude from har: ${s}`) : a.get(s) && a.get(s) === n || (o && s.startsWith(o) || s.startsWith(r) ? a.set(s, n) : (a.set(s, n), await file_util_js_1.FileUtil.linkFile(s, n)));
    }
  }
  isExcludeFromHar(e) {
    const {
      excludeFromHar: t = true
    } = this.targetService.getNativeLibOption();
    return this.moduleModel.isHarModule() && t && this.excludeFromHar(e);
  }
  async removeHarOutPutSo(e, t) {
    fs_extra_1.existsSync(e) && fs_extra_1.existsSync(t) && this.isExcludeFromHar(e) && (fs_extra_1.rmSync(t), this._log.debug(`Remove the so cache file: ${t}`));
  }
  async syncStl(e, t, i, a) {
    const s = cmake_util_js_1.CmakeUtil.parseStlFromCMakeCache(i);
    this._log.debug(`OHOS_STL is ${s.value}`);
    const r = "c++_static" === s.value || "none" === s.value,
      o = path_1.resolve(t, "libc++.so"),
      n = path_1.resolve(t, common_const_js_1.NativeConst.SHARED_STL_LIBRARY),
      l = path_1.resolve(a.getModuleBuildIntermediates(), "libs", e);
    if (r) return this._log.debug(`${this.moduleName} remove files start`), fs_extra_1.removeSync(o), fs_extra_1.removeSync(n), fs_extra_1.removeSync(l), void this._log.debug(`${this.moduleName} remove files end`);
    this._log.debug(`Copy STL ${common_const_js_1.NativeConst.SHARED_STL_LIBRARY} to ${t} due to OHOS_STL is 'c++_shared'`);
    const u = path_1.resolve(this.sdkInfo.getSdkNativeDir(), "llvm", "lib", cpu_abi_enum_js_1.CpuAbiEnum.getCpuType(e), "c++"),
      d = path_1.resolve(u, common_const_js_1.NativeConst.SHARED_STL_LIBRARY);
    if (!fs_extra_1.existsSync(d)) return void this._log.warn(`Missing stl library ${d};`);
    const _ = fs_extra_1.statSync(d);
    if (fs_extra_1.existsSync(n)) {
      const e = fs_extra_1.statSync(n);
      if (_.size === e.size && _.mtimeMs === e.mtimeMs) return;
    }
    await fs_extra_1.copy(d, n, {
      overwrite: true,
      recursive: true,
      preserveTimestamps: true
    });
  }
  excludeFromHsp(e) {
    return !!this.service.getDependencyInfo().getHspDependencies().map(e => e.getDependencyRootPath()).find(t => path_1.normalize(e).startsWith(path_1.normalize(t)));
  }
  excludeFromHar(e) {
    return e = path_1.normalize(fs_extra_1.realpathSync(e)), !!this.service.getHarDependencies().find(t => {
      const i = path_1.normalize(t.getDependencyRootPath()),
        a = path_1.relative(i, e);
      return !a.startsWith("..") && !path_1.isAbsolute(a);
    });
  }
  getCurrentBuildInfo() {
    var e;
    const t = this.targetService.getBuildMode();
    let i = cmake_util_js_1.CmakeUtil.getConfigCMakeBuildType(t);
    const a = cmake_util_js_1.CmakeUtil.getCMakeArguments(null === (e = this._nativeOption) || undefined === e ? undefined : e.arguments).flatMap(e => e.trim().split(" "));
    if (a.length > 0) {
      const e = cmake_util_js_1.CmakeUtil.parseCmakeArgs(a, undefined).get("-DCMAKE_BUILD_TYPE");
      e && e.getValue() && (i = e.getValue());
    }
    return {
      buildMode: t,
      cmakeBuildType: i
    };
  }
  async checkAndHandleBuildState(e, t) {
    const i = this.getCurrentBuildInfo();
    if (!i) return;
    const a = path_1.resolve(e.getIntermediatesBuildInfo(), "build_info.json");
    if (!fs_extra_1.existsSync(a)) return;
    const s = await fs_extra_1.readJSON(a),
      r = s.buildMode === i.buildMode,
      o = s.cmakeBuildType === i.cmakeBuildType;
    if (!r && !o) {
      const i = e.getIntermediatesCppOutPut();
      await this.removeDirectoriesByFilter(i, e => t.includes(e));
    }
  }
  async cacheBuildInfo(e) {
    const t = path_1.resolve(e.getIntermediatesBuildInfo(), "build_info.json"),
      i = this.getCurrentBuildInfo();
    try {
      fs_extra_1.existsSync(t) || (await fs_extra_1.mkdir(path_1.dirname(t), {
        recursive: true
      })), await fs_extra_1.writeJSON(t, i);
    } catch (e) {
      e instanceof Error && this._log.printErrorExit("FILE_WRITE_FAILED", [e.message, t]);
    }
  }
}
