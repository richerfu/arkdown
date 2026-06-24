/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/restool-file-config-builder.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as file_util_js_1 from "../utils/file-util";
export class RestoolConfigBuilder {
  constructor() {
    this.inputFiles = new hvigor_1.FileSet(), this.outputFiles = new hvigor_1.FileSet(), this.existsResourceDir = false, this.resToolConfigObj = {
      configPath: undefined,
      packageName: undefined,
      output: undefined,
      startId: undefined,
      moduleNames: undefined,
      ResourceTable: [],
      applicationResource: undefined,
      moduleResources: [],
      dependencies: [],
      entryCompiledResource: undefined,
      iconCheck: false,
      compression: undefined,
      thread: undefined,
      ignoreResourcePattern: undefined,
      qualifiersConfig: undefined
    };
  }
  getConfigObject() {
    return this.resToolConfigObj;
  }
  getResourceDir() {
    if (!this.existsResourceDir) return [];
    const e = [...this.resToolConfigObj.moduleResources, ...this.resToolConfigObj.dependencies];
    return this.resToolConfigObj.applicationResource && e.push(this.resToolConfigObj.applicationResource), e;
  }
  addInputVariantDir(e) {
    return this;
  }
  addInputDir(e, i) {
    if (file_util_js_1.FileUtil.checkPathLength(e), i) {
      if (!fs_extra_1.existsSync(e)) return this;
      if (this.inputFiles.addEntry(e), i === build_directory_const_js_1.ResToolInputTypeConst.APP) return this.addAppInputDir(e);
      if (i === build_directory_const_js_1.ResToolInputTypeConst.MODULE) return this.addModuleInputDir(e);
      if (i === build_directory_const_js_1.ResToolInputTypeConst.HAR) return this.addHarInputDir(e);
    }
    return this;
  }
  addAppInputDir(e) {
    return file_util_js_1.FileUtil.checkPathLength(e), this.existsResourceDir = true, this.resToolConfigObj.applicationResource = e, this;
  }
  addModuleInputDir(e) {
    return file_util_js_1.FileUtil.checkPathLength(e), this.existsResourceDir = true, -1 === this.resToolConfigObj.moduleResources.indexOf(e) && this.resToolConfigObj.moduleResources.push(e), this;
  }
  addHarInputDir(e) {
    return file_util_js_1.FileUtil.checkPathLength(e), this.existsResourceDir = true, -1 === this.resToolConfigObj.dependencies.indexOf(e) && this.resToolConfigObj.dependencies.push(e), this;
  }
  addJsonFile(e) {
    return file_util_js_1.FileUtil.checkPathLength(e), this.resToolConfigObj.configPath = e, this.inputFiles.addEntry(e, {
      isDirectory: false
    }), this;
  }
  addModulePackName(e) {
    return this.resToolConfigObj.packageName = e, this;
  }
  addEntryCompiledResource(e) {
    return file_util_js_1.FileUtil.checkPathLength(e), this.resToolConfigObj.entryCompiledResource = e, this;
  }
  addOutputDir(e) {
    return file_util_js_1.FileUtil.checkPathLength(e), this.resToolConfigObj.output = e, this.outputFiles.addEntry(e), this;
  }
  addIdsPath(e) {
    return file_util_js_1.FileUtil.checkPathLength(e), this.resToolConfigObj.ids = e, this;
  }
  addDefinedIds(e) {
    return file_util_js_1.FileUtil.checkPathLength(e), this.resToolConfigObj.definedIds = e, this;
  }
  addDefinedSysIds(e) {
    return file_util_js_1.FileUtil.checkPathLength(e), this.resToolConfigObj.definedSysIds = e, this;
  }
  addResTable(e) {
    return file_util_js_1.FileUtil.checkPathLength(e), this.resToolConfigObj.ResourceTable.includes(e) || (this.resToolConfigObj.ResourceTable.push(e), this.outputFiles.addEntry(e, {
      isDirectory: false
    })), this;
  }
  setIconCheck(e) {
    return this.resToolConfigObj.iconCheck = e, this;
  }
  setResCompileThread(e) {
    return e && (this.resToolConfigObj.thread = e), this;
  }
  setIgnoreResourcePattern(e) {
    return e && Array.isArray(e) && (this.resToolConfigObj.ignoreResourcePattern = e), this;
  }
  setQualifiersConfig(e, i) {
    if (e || undefined === i) return this;
    let t = [];
    undefined !== i["Mcc&Mnc"] && (t = i["Mcc&Mnc"], delete i["Mcc&Mnc"]);
    const s = {
      ...i
    };
    return t.length > 0 && (s.MccMnc = t), this.resToolConfigObj.qualifiersConfig = s, this;
  }
  forceDelete() {
    return this;
  }
  incremental() {
    return this;
  }
  compressImage() {
    return this;
  }
  addOutputBak(e) {
    return this;
  }
  addModules(e) {
    return this.resToolConfigObj.moduleNames = e, this;
  }
  addStartId(e) {
    return this.resToolConfigObj.startId = e, this;
  }
  addAppScopeResourcesDir(e) {
    return this;
  }
  addHarResourcesMap(e) {
    return this;
  }
  addModuleResourcesMap(e) {
    return this;
  }
  incrementalCompile() {
    return this;
  }
  addCompression(e) {
    return this.resToolConfigObj.compression = e, this;
  }
}
