/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/restool-command-builder.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as fse from "fs-extra";
import path_1 from "path";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as file_util_js_1 from "../utils/file-util";
import * as base_command_builder_js_1 from "./base-command-builder";
export class RestoolCommandBuilder extends base_command_builder_js_1.BaseCommandBuilder {
  constructor(t) {
    super([]), this.inputFiles = new hvigor_1.FileSet(), this.outputFiles = new hvigor_1.FileSet(), this.existsResourceDir = false, this.restoolLinkParamObj = {
      appScopeResources: undefined,
      moduleResourcesMap: new Map(),
      harResourcesMap: new Map(),
      outputPath: undefined,
      linkCommand: []
    }, this.commandList.push(t), this.restoolLinkParamObj.linkCommand.push(t);
  }
  addInputVariantDir(t) {
    return fse.existsSync(t) ? (file_util_js_1.FileUtil.checkPathLength(t), this.commandList.push("-b"), this.commandList.push(t), this) : this;
  }
  addInputDir(t, i) {
    return fse.existsSync(t) ? (file_util_js_1.FileUtil.checkPathLength(t), this.existsResourceDir = true, this.commandList.push("-i"), this.commandList.push(t), this) : this;
  }
  addEntryCompiledResource(t) {
    return this;
  }
  addAppScopeResourcesDir(t) {
    return fse.existsSync(t) ? (file_util_js_1.FileUtil.checkPathLength(t), this.inputFiles.addEntry(t), this.existsResourceDir = true, this.restoolLinkParamObj.appScopeResources = t, this) : this;
  }
  addModuleResourcesMap(t) {
    const i = new Map();
    return t.forEach((t, e) => {
      fse.existsSync(e) && (file_util_js_1.FileUtil.checkPathLength(e), this.inputFiles.addEntry(e), i.set(e, t), this.existsResourceDir = true);
    }), this.restoolLinkParamObj.moduleResourcesMap = i, this;
  }
  addHarResourcesMap(t) {
    const i = new Map();
    return t.forEach((t, e) => {
      if (!fse.existsSync(e)) return;
      file_util_js_1.FileUtil.checkPathLength(e);
      fse.statSync(e).isDirectory() ? this.inputFiles.addEntry(e) : this.inputFiles.addEntry(e, {
        isDirectory: false
      }), i.set(e, t), this.existsResourceDir = true;
    }), this.restoolLinkParamObj.harResourcesMap = i, this;
  }
  addJsonFile(t) {
    return file_util_js_1.FileUtil.checkPathLength(t), this.inputFiles.addEntry(t, {
      isDirectory: false
    }), this.commandList.push("-j"), this.commandList.push(t), this.restoolLinkParamObj.linkCommand.push("-j"), this.restoolLinkParamObj.linkCommand.push(t), this;
  }
  addModulePackName(t) {
    return this.commandList.push("-p"), this.commandList.push(t), this.restoolLinkParamObj.linkCommand.push("-p"), this.restoolLinkParamObj.linkCommand.push(t), this;
  }
  addOutputDir(t) {
    return file_util_js_1.FileUtil.checkPathLength(t), this.outputFiles.addEntry(t), this.commandList.push("-o"), this.commandList.push(t), this.restoolLinkParamObj.outputPath = t, this;
  }
  addIdsPath(t) {
    return file_util_js_1.FileUtil.checkPathLength(t), this.restoolLinkParamObj.linkCommand.push("--ids"), this.restoolLinkParamObj.linkCommand.push(t), this.restoolLinkParamObj.idsMapPath = t, this;
  }
  addResTable(t) {
    return file_util_js_1.FileUtil.checkPathLength(t), this.commandList.includes(t) || (this.outputFiles.addEntry(t, {
      isDirectory: false
    }), this.commandList.push("-r"), this.commandList.push(t), this.restoolLinkParamObj.linkCommand.push("-r"), this.restoolLinkParamObj.linkCommand.push(t)), this;
  }
  forceDelete() {
    return this.commandList.push("-f"), this.restoolLinkParamObj.linkCommand.push("-f"), this;
  }
  incremental() {
    return this.commandList.push("-c"), this;
  }
  compressImage() {
    return this.commandList.push("-z"), this;
  }
  addOutputBak(t) {
    return file_util_js_1.FileUtil.checkPathLength(t), this.commandList.push("-n"), this.commandList.push(t), this;
  }
  addModules(t) {
    return this.commandList.push("-m"), this.commandList.push(t), this.restoolLinkParamObj.linkCommand.push("-m"), this.restoolLinkParamObj.linkCommand.push(t), this;
  }
  addStartId(t) {
    return this;
  }
  setIconCheck(t) {
    return this;
  }
  incrementalCompile() {
    return this.restoolLinkParamObj.linkCommand.push("-z"), this;
  }
  addDefinedIds(t) {
    return file_util_js_1.FileUtil.checkPathLength(t), this.restoolLinkParamObj.linkCommand.push("--defined-ids"), this.restoolLinkParamObj.linkCommand.push(path_1.resolve(t, build_directory_const_js_1.BuildArtifactConst.ID_DEFINED_JSON)), this;
  }
  addDefinedSysIds(t) {
    return this;
  }
}
