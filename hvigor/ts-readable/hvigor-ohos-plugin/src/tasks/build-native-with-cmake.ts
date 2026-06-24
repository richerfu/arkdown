/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/build-native-with-cmake.js
 */
import os_1 from "os";
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as native_command_builder_js_1 from "../builder/native-command-builder";
import * as ohos_trace_js_1 from "../common/trace/ohos-trace";
import * as common_const_js_1 from "../const/common-const";
import * as sdk_const_js_1 from "../const/sdk-const";
import * as hvigor_native_build_js_1 from "../model/cxx/hvigor-native-build";
import * as cmake_invalidation_state_js_1 from "../utils/cmake/cmake-invalidation-state";
import * as cmake_util_js_1 from "../utils/cmake/cmake-util";
import * as file_util_js_1 from "../utils/file-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as abstract_build_native_js_1 from "./abstract-build-native";
import * as task_names_js_1 from "./common/task-names";
import * as legacy_pre_build_js_1 from "./legacy-tasks/legacy-pre-build";
import * as pre_build_js_1 from "./pre-build";
import * as build_mode_const_1 from "../const/build-mode-const";
import * as type_js_1 from "../common/trace/type";
var Task = task_names_js_1.TaskNames.Task;
export class BuildNativeWithCmake extends abstract_build_native_js_1.AbstractBuildNative {
  constructor(e) {
    super(e, Task.BUILD_NATIVE_WITH_CMAKE), this._log = ohos_logger_js_1.OhosLogger.getLogger(BuildNativeWithCmake.name), this._moduleDir = this.moduleModel.getProjectDir();
  }
  initTaskDepends() {
    this.declareDepends(this.isFaMode ? legacy_pre_build_js_1.LegacyPreBuild.name : pre_build_js_1.PreBuild.name);
  }
  taskShouldDo() {
    return cmake_util_js_1.CmakeUtil.nativeTaskCondition(this.moduleModel, this.targetData, this._nativeOption);
  }
  async doTaskAction() {
    var e;
    if (file_util_js_1.FileUtil.checkDirWithoutDelete(this.pathInfo.getModuleBuildPath()), file_util_js_1.FileUtil.checkDirWithoutDelete(this.pathInfo.getCppOutputDir()), undefined !== this._nativeOption) {
      let e = this._nativeOption.path;
      undefined === e && (e = "./src/main/cpp/CMakeLists.txt", this._log.warn(`The path of the cmake script in externalNativeOptions is empty. Use the default path ${e}.`));
    }
    const t = cmake_util_js_1.CmakeUtil.checkAbiFilters(null === (e = this._nativeOption) || undefined === e ? undefined : e.abiFilters, this.targetData.isHarmonyOS(), this.moduleModel, this.targetName);
    for (const e of t) {
      const t = `generate CMake command with ABI of ${e}`,
        i = this.durationEvent.createSubEvent(t, "");
      i.start();
      const a = path_1.resolve(this.pathInfo.getNinjaWorkDir(), e),
        s = this.buildCommand(e),
        o = path_1.resolve(a, "metadata_generation_command.txt"),
        n = s.join(os_1.EOL);
      await this.ensureCommandFile(o, n), i.stop(), i.setLog(t, hvigor_1.MetricLogType.INFO);
      const r = `submit CMake task to work pool with ABI of ${e}`,
        l = this.durationEvent.createSubEvent(r, "");
      l.start();
      const _ = path_1.resolve(a, "configure_fingerprint.json"),
        d = await (0, cmake_invalidation_state_js_1.createInvalidationState)(cmake_invalidation_state_js_1.FileId.getInstance(_), await this.getInputFiles(a, e), this.getRequiredOutputFiles(a), this.getOptionalOutputFiles(a), this.getHardOutputFiles(a)),
        u = d.shouldConfigure();
      if (ohos_trace_js_1.ohosTrace.traceIncrement(this.moduleName, type_js_1.TraceIncrement.BUILD_NATIVE_WITH_CMAKE, !u), u) {
        d.softConfigureOkay() || (this._log.debug("Clean CMakeCache.txt due to outdated CMake configuration."), await fs_extra_1.rm(path_1.resolve(a, "CMakeCache.txt"), {
          recursive: true,
          force: true
        }));
        const t = [a, e, o],
          i = this.getCmakeCallback(t, d, l, r);
        d.shouldConfigureMessages().forEach(e => this._log.debug(e)), cmake_util_js_1.CmakeUtil.mkCodeModelRequest(a), await this.executeCommand(s, a, i, [], l);
      } else this._log.debug("CMake configuration was up-to-date"), await d.updateFingerPrint(), l.stop(), l.setLog(r, hvigor_1.MetricLogType.INFO);
    }
  }
  async ensureCommandFile(e, t) {
    try {
      if (fs_extra_1.existsSync(e)) {
        (await fs_extra_1.readFile(e, {
          encoding: "utf-8"
        })) !== t && (await fs_extra_1.writeFile(e, t));
      } else await fs_extra_1.mkdir(path_1.dirname(e), {
        recursive: true
      }), await fs_extra_1.writeFile(e, t);
    } catch (t) {
      this._log.printErrorExit("FILE_WRITE_FAILED", [t.message, e]);
    }
  }
  getCmakeCallback(e, t, i, a) {
    return async () => {
      var s, o;
      const n = e[0],
        r = e[1],
        l = e[2],
        _ = (0, cmake_invalidation_state_js_1.readCMakeFileApiReply)(n, this.targetName, r, this.sdkInfo.getCmakeTool()),
        d = null !== (o = null === (s = _.extra) || undefined === s ? undefined : s.join(os_1.EOL)) && undefined !== o ? o : "";
      try {
        await fs_extra_1.writeFile(path_1.resolve(n, "additional_project_files.txt"), d), await fs_extra_1.writeFile(path_1.resolve(n, "native_work_dir.txt"), n), await fs_extra_1.writeFile(path_1.resolve(n, "build_file_index.txt"), _.buildFiles.join(os_1.EOL)), delete _.extra, await fs_extra_1.writeJSON(path_1.resolve(n, hvigor_native_build_js_1.HvigorNativeConfigFile), _);
      } catch (e) {
        e instanceof Error && this._log.printErrorExit("FILE_WRITE_FAILED", [e.message, l]);
      }
      t.addInputFiles(await this.getInputFiles(n, r)), await t.updateFingerPrint(), i.stop(), i.setLog(a, hvigor_1.MetricLogType.INFO);
    };
  }
  async getInputFiles(e, t) {
    const i = new Set();
    i.add(path_1.resolve(cmake_util_js_1.CmakeUtil.getCmakeListDir(this.moduleModel, this.targetName, this._nativeOption), "CMakeLists.txt")), file_util_js_1.FileUtil.getAllFilesFromFolder(path_1.resolve(this.pathInfo.getNinjaWorkDir(), "hvigor", t)).forEach(e => i.add(e));
    const a = path_1.resolve(e, hvigor_native_build_js_1.HvigorNativeConfigFile);
    if (!fs_extra_1.existsSync(a)) return [...i].map(e => cmake_invalidation_state_js_1.FileId.getInstance(e));
    const s = await (0, hvigor_native_build_js_1.readHvigorNativeConfigFile)(a);
    return null == s || s.buildFiles.forEach(e => i.add(e)), [...i].map(e => cmake_invalidation_state_js_1.FileId.getInstance(e));
  }
  getOptionalOutputFiles(e) {
    return [path_1.resolve(e, "build.ninja"), path_1.resolve(e, "compile_commands.json"), path_1.resolve(e, "native_work_dir.txt"), path_1.resolve(e, "build_file_index.txt"), path_1.resolve(e, "additional_project_files.txt")].map(e => cmake_invalidation_state_js_1.FileId.getInstance(e));
  }
  getRequiredOutputFiles(e) {
    return [path_1.resolve(e, hvigor_native_build_js_1.HvigorNativeConfigFile), path_1.resolve(e, "CMakeCache.txt")].map(e => cmake_invalidation_state_js_1.FileId.getInstance(e));
  }
  getHardOutputFiles(e) {
    return [path_1.resolve(e, "metadata_generation_command.txt")].map(e => cmake_invalidation_state_js_1.FileId.getInstance(e));
  }
  getToolchainFile(e, t, i) {
    let a = "";
    return e === common_const_js_1.CommonCppConst.BISHENG ? (t || this._log.warn(`Unknown compiler BiSheng, The runtimeOS that supports the BiSheng can only be HarmonyOS on the ${i} architecture.`), a = t ? this.sdkInfo.getHmsBiShengToolchainFile() : this.sdkInfo.getNativeToolchain()) : a = t ? this.sdkInfo.getHmsToolchainFile() : this.sdkInfo.getNativeToolchain(), fs_extra_1.existsSync(a) || (this._log.warn(`Cannot find the file under the specified path ${a}`), a = fs_extra_1.existsSync(this.sdkInfo.getHmsToolchainFile()) ? this.sdkInfo.getHmsToolchainFile() : this.sdkInfo.getNativeToolchain()), a;
  }
  buildCommand(e) {
    var t, i, a;
    const s = new native_command_builder_js_1.NativeCommandBuilder(this.sdkInfo.getCmakeTool()),
      o = path_1.resolve(this.pathInfo.getIntermediatesCppOutPut(), e),
      n = path_1.resolve(this.pathInfo.getNinjaWorkDir(), e),
      r = path_1.resolve(this.pathInfo.getNinjaWorkDir(), "hvigor", e),
      l = this.targetService.getBuildMode(),
      _ = this.targetData.isHarmonyOS() && this.compileApiVersion > 9,
      d = null === (t = this.targetService.getBuildOption()) || undefined === t ? undefined : t.nativeCompiler,
      u = this.getToolchainFile(d, _, e),
      h = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.OHOS_TEST_COVERAGE);
    s.addCmakeList(cmake_util_js_1.CmakeUtil.getCmakeListDir(this.moduleModel, this.targetName, this._nativeOption)).addTempFilePath(n).addOhosArch(e).addOutputDir(o).addBuildType(l).addNativeSdk(this.sdkInfo.getSdkNativeDir()).addSystemName("OHOS").addOhosArchAbi(e).exportCompileCommands("ON").addToolChain(u).addGenerator("Ninja").addMakeProgramPath(this.sdkInfo.getNativeNinjaTool()).addCMakeFindPath(r).addPackageFindScript(path_1.resolve(r, common_const_js_1.NativeConst.PACKAGE_SUMMARY_FILE)).muteUnusedCliWarn();
    let c = (null === (i = this._nativeOption) || undefined === i ? undefined : i.cFlags) || "",
      m = (null === (a = this._nativeOption) || undefined === a ? undefined : a.cppFlags) || "";
    this.isEnableCppIncrementalBuildArg(e) && (c = this.addCppIncrementalBuildArg(c), m = this.addCppIncrementalBuildArg(m)), c && "" !== c.trim() && s.addCFlags(c), m && "" !== m.trim() && s.addCxxFlags(m), undefined !== h && s.addOhosCoverage(this.getTestCoverageArg(h)), _ && s.addHMosNativeSdk(this.sdkInfo.getHmsNativeDir());
    const g = this.addNativeArgs(e);
    let p = s.build();
    return p = cmake_util_js_1.CmakeUtil.mergeCommandLine(p, g), this._log._printDebugCommand("Cmake", p), p;
  }
  getTestCoverageArg(e) {
    return "true" === e && this.targetService.isOhosTest() ? "TRUE" : "FALSE";
  }
  isEnableCppIncrementalBuildArg(e) {
    var t, i, a;
    if ("true" !== process.env.ENABLE_CPP_FUNCTION_LEVEL_INCREMENTAL) return false;
    const s = (null === (i = null === (t = this.targetService.getModuleService()) || undefined === t ? undefined : t.getModuleModel()) || undefined === i ? undefined : i.getName()) || "unknown";
    return this.targetService.getBuildMode() !== build_mode_const_1.BuildMode.DEBUG ? (this._log.warn(`The C++ function-level incremental compilation is not supported in Release mode on the ${e} architecture. The relevant module is '${s}'.`), false) : (null === (a = this.targetService.getBuildOption()) || undefined === a ? undefined : a.nativeCompiler) === common_const_js_1.CommonCppConst.BISHENG || (this._log.warn(`The C++ function-level incremental compilation is only supported for BiSheng compiler on the ${e} architecture. The relevant module is '${s}'.`), false);
  }
  addCppIncrementalBuildArg(e) {
    return e.includes(common_const_js_1.CommonCppConst.ICLANG_COMPILE_FLAG) ? e : `${e} ${common_const_js_1.CommonCppConst.ICLANG_COMPILE_FLAG}`;
  }
  addNativeArgs(e) {
    var t;
    const i = cmake_util_js_1.CmakeUtil.getCMakeArguments(null === (t = this._nativeOption) || undefined === t ? undefined : t.arguments);
    new Map([["ohos-debug-asan", "-DOHOS_ENABLE_ASAN=ON"], ["ohos-enable-tsan", "-DOHOS_ENABLE_TSAN=ON"], ["ohos-enable-hwasan", "-DOHOS_ENABLE_HWASAN=ON"], ["ohos-enable-ubsan", "-DOHOS_ENABLE_UBSAN=ON"]]).forEach((t, a) => {
      if ("ohos-enable-hwasan" === a && "arm64-v8a" !== e) return;
      "true" === hvigor_1.hvigorCore.getExtraConfig().get(a) && i.push(t);
    });
    const a = i.flatMap(e => e.trim().split(" "));
    return this.checkSanArgsSupported(a), this.checkExclusiveSan(a), a;
  }
  checkSanArgsSupported(e) {
    const t = this.targetData.getApiMeta();
    e.includes("-DOHOS_ENABLE_ASAN=ON") && t.compatibleSdkVersion.version <= sdk_const_js_1.ApiVersion.API_VERSION_9 && this._log.warn("To use ASan, first upgrade the SDK to API version 10 or later.");
    e.some(e => BuildNativeWithCmake.sanAbilitySet.has(e) && "-DOHOS_ENABLE_ASAN=ON" !== e) && t.compatibleSdkVersion.version <= sdk_const_js_1.ApiVersion.API_VERSION_11 && this._log.warn("To use TSan, HWASan, or UBSan, first upgrade the SDK to API version 12 or later.");
  }
  checkExclusiveSan(e) {
    new Set(e.filter(e => BuildNativeWithCmake.sanAbilitySet.has(e))).size > 1 && this._log.warn("Sanitizers are mutually exclusive. Enable only one of them, such as ASan, TSan, HWASan, or UBSan, when using CMake.");
  }
}
BuildNativeWithCmake.sanAbilitySet = new Set(["-DOHOS_ENABLE_ASAN=ON", "-DOHOS_ENABLE_TSAN=ON", "-DOHOS_ENABLE_HWASAN=ON", "-DOHOS_ENABLE_UBSAN=ON"]);
