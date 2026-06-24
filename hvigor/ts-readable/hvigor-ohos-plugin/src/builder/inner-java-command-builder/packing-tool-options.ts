/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/inner-java-command-builder/packing-tool-options.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as common_const_1 from "../../const/common-const";
import * as file_util_js_1 from "../../utils/file-util";
import * as java_command_builder_js_1 from "../java-command-builder";
export class PackingToolOptions extends java_command_builder_js_1.JavaCommandBuilder {
  constructor() {
    super(), this.setJavaSystemProperty("file.encoding", (0, hvigor_1.isWindows)() ? "GBK" : "utf-8");
  }
  addFieldAndPath(t, d) {
    return file_util_js_1.FileUtil.checkPathLength(d), this.commandList.push(t), this.commandList.push(d), this;
  }
  addMode(t) {
    return this.commandList.push("--mode"), this.commandList.push(t), this;
  }
  addHapPath(t) {
    return t.split(",").forEach(file_util_js_1.FileUtil.checkPathLength), this.commandList.push("--hap-path"), this.commandList.push(t), this;
  }
  addHspPath(t) {
    return t.split(",").forEach(file_util_js_1.FileUtil.checkPathLength), this.commandList.push("--hsp-path"), this.commandList.push(t), this;
  }
  addAppPath(t) {
    return t.split(",").forEach(file_util_js_1.FileUtil.checkPathLength), this.commandList.push("--app-path"), this.commandList.push(t), this;
  }
  addInputList(t) {
    return t.split(",").forEach(file_util_js_1.FileUtil.checkPathLength), this.commandList.push("--input-list"), this.commandList.push(t), this;
  }
  addDeviceTypes(t) {
    return this.commandList.push("--device-types"), this.commandList.push(t), this;
  }
  addHarPath(t) {
    return this.addFieldAndPath("--har-path", t);
  }
  addPackInfoPath(t) {
    return this.addFieldAndPath("--pack-info-path", t);
  }
  addPkgContextInfoPath(t) {
    return this.addFieldAndPath("--pkg-context-path", t);
  }
  addPkgSdkInfoPath(t) {
    return this.addFieldAndPath("--pkg-sdk-info-path", t);
  }
  addHspList(t) {
    return this.addFieldAndPath("--hsp-list", t);
  }
  addVersionCode(t) {
    return this.addFieldAndPath("--version-code", t);
  }
  addBuildVersion(t) {
    return undefined === t ? this : this.addFieldAndPath("--build-version", t);
  }
  addBundleName(t) {
    return this.addFieldAndPath("--bundle-name", t);
  }
  addDirList(t) {
    for (const d of t) file_util_js_1.FileUtil.checkPathLength(d);
    return this.commandList.push("--dir-list"), this.commandList.push(t.join(",")), this;
  }
  addOutPath(t) {
    return this.addFieldAndPath("--out-path", t);
  }
  addFilePath(t) {
    return this.addFieldAndPath("--file-path", t);
  }
  addProjectPath(t) {
    return this.addFieldAndPath("--project-path", t);
  }
  addBinPath(t) {
    return this.addFieldAndPath("--bin-path", t);
  }
  addJsonPath(t) {
    return this.addFieldAndPath("--json-path", t);
  }
  addPackResPath(t) {
    return this.addFieldAndPath("--pack-res-path", t);
  }
  addEntryCardPath(t) {
    return this.addFieldAndPath("--entrycard-path", t);
  }
  addResourcesPath(t) {
    return this.addFieldAndPath("--resources-path", t);
  }
  addResourceTableTxtPath(t) {
    return this.addFieldAndPath("--txt-path", t);
  }
  addJsPath(t) {
    return this.addFieldAndPath("--js-path", t);
  }
  addEtsPath(t) {
    return this.addFieldAndPath("--ets-path", t);
  }
  addAssetsPath(t) {
    return this.addFieldAndPath("--assets-path", t);
  }
  addIndexPath(t) {
    return this.addFieldAndPath("--index-path", t);
  }
  addLibPath(t) {
    return this.addFieldAndPath("--lib-path", t);
  }
  addJarPath(t) {
    return this.addFieldAndPath("--jar-path", t);
  }
  addAnPath(t) {
    return this.addFieldAndPath("--an-path", t);
  }
  addSysCapPath(t) {
    return this.addFieldAndPath("--rpcid-path", t);
  }
  addMainModuleLimit(t) {
    return this.commandList.push("--main-module-limit"), this.commandList.push(t.toString()), this;
  }
  addNormalModuleLimit(t) {
    return this.commandList.push("--normal-module-limit"), this.commandList.push(t.toString()), this;
  }
  addTotalLimit(t) {
    return this.commandList.push("--total-limit"), this.commandList.push(t.toString()), this;
  }
  addCompressLevel(t, d) {
    const i = common_const_1.HvigorConfigConst.OHOS_PACK_COMPRESS_LEVEL_MAP.get(t);
    return d && d < 12 || i && (this.commandList.push("--compress-level"), this.commandList.push(i)), this;
  }
  addPacArg(t) {
    return this.commandList.push("--pac-json-path"), this.commandList.push(t), this;
  }
  force(t) {
    return this.commandList.push("--force"), this.commandList.push(t.toString()), this;
  }
  replacePackInfo(t) {
    return this.commandList.push("--replace-pack-info"), this.commandList.push(t.toString()), this;
  }
}
