/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/native-command-builder.js
 */
import fs_1 from "fs";
import * as cmake_util_js_1 from "../utils/cmake/cmake-util";
import * as file_util_js_1 from "../utils/file-util";
import * as base_command_builder_js_1 from "./base-command-builder";
import * as ohos_logger_1 from "../utils/log/ohos-logger";
export class NativeCommandBuilder extends base_command_builder_js_1.BaseCommandBuilder {
  constructor(t) {
    super([]), this.log = ohos_logger_1.OhosLogger.getLogger(NativeCommandBuilder.name), this.commandList.push(t);
  }
  addCmakeList(t) {
    return file_util_js_1.FileUtil.checkPathLength(t), this.commandList.push(`-H${t}`), this;
  }
  addTempFilePath(t) {
    return file_util_js_1.FileUtil.checkPathLength(t), this.commandList.push(`-B${t}`), this;
  }
  addOhosArch(t) {
    return this.commandList.push(`-DOHOS_ARCH=${t}`), this;
  }
  addOutputDir(t) {
    return file_util_js_1.FileUtil.checkPathLength(t), this.commandList.push(`-DCMAKE_LIBRARY_OUTPUT_DIRECTORY=${t}`), this;
  }
  addCMakeFindPath(t) {
    return file_util_js_1.FileUtil.checkPathLength(t), fs_1.existsSync(t) && this.commandList.push(`-DCMAKE_FIND_ROOT_PATH=${t}`), this;
  }
  addPackageFindScript(t) {
    return file_util_js_1.FileUtil.checkPathLength(t), fs_1.existsSync(t) && this.commandList.push(`-DPACKAGE_FIND_FILE=${t}`), this;
  }
  addBuildType(t) {
    return this.commandList.push(`-DCMAKE_BUILD_TYPE=${cmake_util_js_1.CmakeUtil.getConfigCMakeBuildType(t)}`), this;
  }
  addNativeSdk(t) {
    return file_util_js_1.FileUtil.checkPathLength(t), this.commandList.push(`-DOHOS_SDK_NATIVE=${t}`), this;
  }
  addHMosNativeSdk(t) {
    return file_util_js_1.FileUtil.checkPathLength(t), this.commandList.push(`-DHMOS_SDK_NATIVE=${t}`), this;
  }
  addSystemName(t) {
    return this.commandList.push(`-DCMAKE_SYSTEM_NAME=${t}`), this;
  }
  addOhosArchAbi(t) {
    return this.commandList.push(`-DCMAKE_OHOS_ARCH_ABI=${t}`), this;
  }
  exportCompileCommands(t) {
    return this.commandList.push(`-DCMAKE_EXPORT_COMPILE_COMMANDS=${t}`), this;
  }
  addToolChain(t) {
    return file_util_js_1.FileUtil.checkPathLength(t), this.commandList.push(`-DCMAKE_TOOLCHAIN_FILE=${t}`), this;
  }
  addGenerator(t) {
    return this.commandList.push(`-G${t}`), this;
  }
  addMakeProgramPath(t) {
    return file_util_js_1.FileUtil.checkPathLength(t), this.commandList.push(`-DCMAKE_MAKE_PROGRAM=${t}`), this;
  }
  addCFlags(t) {
    return this.commandList.push(`-DCMAKE_C_FLAGS=${t}`), this;
  }
  addCxxFlags(t) {
    return this.commandList.push(`-DCMAKE_CXX_FLAGS=${t}`), this;
  }
  addOhosCoverage(t) {
    return this.commandList.push(`-DOHOS_TEST_COVERAGE=${t}`), this;
  }
  changeToDir(t) {
    return file_util_js_1.FileUtil.checkPathLength(t), this.commandList.push("-C"), this.commandList.push(t), this;
  }
  muteUnusedCliWarn() {
    return this.commandList.push("--no-warn-unused-cli"), this;
  }
}
