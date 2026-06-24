/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract/abstract-process-har-artifacts.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as fs from "fs-extra";
import * as minimatch_1 from "minimatch";
import * as project_ohos_config_manager_js_1 from "../../common/global/project-ohos-config-manager";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as cmake_util_js_1 from "../../utils/cmake/cmake-util";
import * as file_util_js_1 from "../../utils/file-util";
import * as oh_package_loader_js_1 from "../../utils/loader/file/oh-package-loader";
import * as npm_utils_js_1 from "../../utils/npm-utils";
import * as obfuscation_config_resolver_js_1 from "../../utils/obfuscation-config-resolver";
import * as abstract_merge_profile_js_1 from "../abstract-merge-profile";
import * as ohos_har_task_js_1 from "../task/ohos-har-task";
var __decorate = this && this.__decorate || function (e, t, i, s) {
  var o,
    r = arguments.length,
    a = r < 3 ? t : null === s ? s = Object.getOwnPropertyDescriptor(t, i) : s;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) a = Reflect.decorate(e, t, i, s);else for (var n = e.length - 1; n >= 0; n--) (o = e[n]) && (a = (r < 3 ? o(a) : r > 3 ? o(t, i, a) : o(t, i)) || a);
  return r > 3 && a && Object.defineProperty(t, i, a), a;
};
export class AbstractProcessHarArtifacts extends ohos_har_task_js_1.OhosHarTask {
  declareExecutionEnv() {
    return new Map().set("cwd", this.taskTmpDir);
  }
  declareInputs() {
    var e;
    const t = new Map().set("hasNativeOption", this.hasNativeOption).set("needCppTypes", this.needCppTypes).set("harModuleJson", this.harModuleJson).set("isOhpmProject", this.isOhpmProject).set("artifactType", this._harExtendInfo.getArtifactType()).set("isByteCodeHar", this.targetService.isByteCodeHar()).set(abstract_merge_profile_js_1.INPUT_KEY.projectConfigAppOpt, JSON.stringify(null === (e = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === e ? undefined : e.signingConfig));
    return this.hasNativeOption && undefined !== this.cmakeListDir && t.set("cmakeListDir", this.cmakeListDir), this.hasNativeOption && this.needCppTypes && t.set("cppTypes", this.cppTypes), t;
  }
  get ignoreResourcePattern() {
    var e;
    return null === (e = this.targetService.getBuildOption().resOptions) || undefined === e ? undefined : e.ignoreResourcePattern;
  }
  declareInputFiles() {
    const e = this.allInvalidPrefix.join("|").replace(/\\/g, "\\\\"),
      t = RegExp(`^(?!${e}).*`),
      i = new hvigor_1.FileSet().addEntry(this.harProfile, {
        isDirectory: true
      }).addEntry(this._moduleDir, {
        isDirectory: true,
        test: t
      }).addEntry(this.generateDir, {
        isDirectory: true
      });
    fs.pathExistsSync(this.processedLibs) && i.addEntry(this.processedLibs, {
      isDirectory: true
    }), fs.pathExistsSync(this.resourceTablePath) && i.addEntry(this.resourceTablePath, {
      isDirectory: false
    });
    for (const e of this.headerPath) {
      const t = path_1.isAbsolute(e) ? e : path_1.resolve(this._moduleDir, e);
      fs.pathExistsSync(t) && i.addEntry(t);
    }
    return i;
  }
  declareOutputFiles() {
    var e;
    const t = new hvigor_1.FileSet().addEntries([this.taskTmpDir], {
      isDirectory: true
    });
    return hvigor_1.coreParameter.properties[hvigor_1.OHOS_ARK_COMPILE_SOURCE_MAP_DIR] || !this.sdkInfo.hasRollUpPluginInEtsLoader || (null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.debuggable) || t.addEntry(path_1.resolve(this.pathInfo.getDefaultSourceMapPath(), build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP), {
      isDirectory: false
    }), t;
  }
  constructor(e, t) {
    var i;
    super(e, t), this.allInvalidPrefix = [], this.ignoreFileNames = [".gitignore", ".ohpmignore"], this.ohpmIgnoreRules = [], this.releaseWhiteListTemDir = [common_const_js_1.CommonConst.RELEASE_HAR_WHITE_LIST_DIR_NAME], this.packingIgnoreFiles = ["/node_modules", "/oh_modules", "/.preview", "/build", "/.cxx", "/.test"], this.needCopyFileNames = [], this._moduleDir = this.moduleModel.getProjectDir(), this.projectDir = this.projectModel.getProjectDir(), this.taskTmpDir = this.getTaskTempDir(this.targetData, this.service.isFaMode() ? `${this.targetService.getTargetData().getTargetName()}@LegacyPackageHar` : `${this.targetService.getTargetData().getTargetName()}@PackageHar`, false), this.generateDir = this.pathInfo.getGeneratePmDir();
    const s = null === (i = this.targetService.getBuildOption().nativeLib) || undefined === i ? undefined : i.headerPath;
    "string" == typeof s ? this.headerPath = [s] : Array.isArray(s) ? this.headerPath = s : this.headerPath = [], this.resourceDir = this.pathInfo.getIntermediatesRes(), this.processedLibs = this.pathInfo.getIntermediatesStrippedLibsDir(), this.resourceTablePath = path_1.resolve(this.resourceDir, build_directory_const_js_1.BuildArtifactConst.RESOURCE_TABLE_TXT), this.harProfile = this.pathInfo.getIntermediatesProcessProfileDir(), this.harModuleJson = path_1.resolve(this.taskTmpDir, "src", "main", common_const_js_1.CommonConst.MODULE_JSON5), cmake_util_js_1.CmakeUtil.nativeTaskCondition(this.moduleModel, this.targetData, this.targetService.getBuildOption().externalNativeOptions) && (this.cmakeListDir = cmake_util_js_1.CmakeUtil.getCmakeListDir(this.moduleModel, this.targetName, this.targetService.getBuildOption().externalNativeOptions)), this.obfuscationFiles = this.resolveObfuscationFiles(), this.cppTypes = path_1.resolve(this._moduleDir, "src", "main", "cpp", "types"), this.hasNativeOption = this._harExtendInfo.hasConfiguredNativeOption(), this.needCppTypes = fs.pathExistsSync(this.cppTypes), this.initModuleToplevelPathFilter(), this.initNeedCopyFileNames();
  }
  initModuleToplevelPathFilter() {
    const e = [build_directory_const_js_1.BuildDirConst.LIBS, build_directory_const_js_1.BuildDirConst.BUILD_ROOT, common_const_js_1.CommonNodeConst.NODE_MODULES, common_const_js_1.CommonConst.OH_MODULES, ".cxx", ".previewer", ".hvigor", ...this.ignoreFileNames];
    e.includes(this.pathInfo.getBuildRoot()) || e.push(this.pathInfo.getBuildRoot()), e.forEach(e => {
      this.allInvalidPrefix.push(path_1.resolve(this._moduleDir, e));
    });
  }
  initNeedCopyFileNames() {
    const e = this.isOhpmProject ? common_const_js_1.CommonConst.OH_PACKAGE_JSON5 : common_const_js_1.CommonConst.PACKAGE_JSON,
      t = oh_package_loader_js_1.ohPackageLoader.getNodeOhPackagePath(path_1.resolve(this._moduleDir, e));
    if (fs.existsSync(t)) {
      const e = hvigor_1.Json5Reader.getJson5Obj(t);
      e.main && this.needCopyFileNames.push(e.main);
    }
    this.needCopyFileNames = this.needCopyFileNames.map(e => path_1.resolve(this._moduleDir, e));
  }
  initFilterRules() {
    var e, t;
    if (!this.isOhpmProject) return;
    const {
      include: i,
      exclude: s
    } = (null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.packingOptions) || undefined === t ? undefined : t.asset) || {};
    if ((null == i ? undefined : i.length) || (null == s ? undefined : s.length)) this.ohpmIgnoreRules.push(...this.packingIgnoreFiles);else for (const e of this.ignoreFileNames) {
      const t = path_1.resolve(this._moduleDir, e);
      if (!fs.existsSync(t) || fs.statSync(t).isDirectory()) continue;
      const i = fs.readFileSync(t).toString().split(/\r?\n/).filter(e => !/^#|^$/.test(e.trim())).map(e => e.trim());
      this.ohpmIgnoreRules.push(...i);
    }
    for (const e of this.headerPath) this.ohpmIgnoreRules.push(e);
    this.ohpmIgnoreRules.push("/src/test");
    this.ohpmIgnoreRules.push("/src/ohosTest"), this.initSourceRootFilterRules();
  }
  initSourceRootFilterRules() {
    var e;
    const t = this.targetData.getTargetOpt(),
      i = null === (e = t.source) || undefined === e ? undefined : e.sourceRoots;
    if (!(null == i ? undefined : i.length)) return;
    const s = t.name,
      o = this.moduleModel.getTargetOptions(),
      r = /^\.[/\\]/,
      a = i.reduce((e, t) => (e.add(t.replace(r, "")), e), new Set());
    o.forEach(e => {
      var t, i;
      e.name !== s && (null === (i = null === (t = e.source) || undefined === t ? undefined : t.sourceRoots) || undefined === i || i.forEach(e => {
        const t = e.replace(r, "");
        a.has(t) || this.ohpmIgnoreRules.push(t);
      }));
    });
  }
  async doTaskAction() {
    var e, t, i, s;
    const o = "verify and initialize packaging information",
      r = this.durationEvent.createSubEvent(o, "");
    r.start(), this.preCheckBeforePack(), this.copyLocalDepsFileToTempDir(), this._harExtendInfo.isObfuscatedHar() || this.targetService.isByteCodeHar() ? (this.copyResourceFileOfReleaseHar(), this.copyModuleSourceFileToTempDirByWhiteList(), this.copyCompiledSourceFileToTempDir(), file_util_js_1.FileUtil.copySpecialFileToTempDir(this._moduleDir, this.taskTmpDir)) : (this.initFilterRules(), this.copyModuleSourceFileToTempDirByBlackList()), file_util_js_1.FileUtil.disposalPackingOptionAsset(this._moduleDir, null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.packingOptions) || undefined === t ? undefined : t.asset, this.taskTmpDir), await this.syncNativeHeader(), await this.syncObfuscationFile(), this.copyBuildIntermediatesOutToTempDir(), this.copyIntermediatePacToTempDir(), this.copyOtherGenerateFiles(), this.processBundleDepRes(), this.processPackageJson(), this.targetService.isCustomizedHar() && this.copyDeclareFileToTempDir(), this.processSourceMaps(), this.processHarRouterMap(), this.processHarResStartupConfig(), this.addPreloadSystemSoJson(), this.processObfuscatedHarAbility();
    const a = null === (s = null === (i = this.targetService.getBuildOption()) || undefined === i ? undefined : i.strictMode) || undefined === s ? undefined : s.useNormalizedOHMUrl,
      n = cmake_util_js_1.CmakeUtil.getHarExtraParams(this);
    let l = new Set();
    const c = this.pathInfo.getIntermediatesProcessLibs();
    fs.existsSync(c) && (l = await file_util_js_1.FileUtil.readDirFileNames(c)), file_util_js_1.FileUtil.addParamToOhPack(this.isOhpmProject, this.taskTmpDir, {
      useNormalizedOHMUrl: a
    }, n, this.targetName, l), r.stop(), r.setLog(o, hvigor_1.MetricLogType.INFO);
  }
  async syncNativeHeader() {
    const e = new Map();
    for (const t of this.headerPath) {
      const i = path_1.isAbsolute(t) ? t : path_1.resolve(this._moduleDir, t);
      cmake_util_js_1.CmakeUtil.checkNativeHeader(i, this.moduleModel, e);
      try {
        await fs.copy(i, path_1.resolve(this.taskTmpDir, "include"), {
          recursive: true
        });
      } catch (e) {
        this.logger.printErrorExit("COPY_DIRECTORY_ERROR", [i]);
      }
    }
  }
  async syncObfuscationFile() {
    var e, t, i;
    const s = null === (e = this.targetService.getBuildOption().arkOptions) || undefined === e ? undefined : e.obfuscation;
    if ((null === (t = null == s ? undefined : s.ruleOptions) || undefined === t ? undefined : t.enable) || (null == s ? undefined : s.consumerFiles) && (null === (i = null == s ? undefined : s.consumerFiles) || undefined === i ? undefined : i.length) > 0) {
      const e = this.pathInfo.getObfuscationExportRulePath();
      try {
        fs.existsSync(e) && (await fs.copyFile(e, path_1.resolve(this.taskTmpDir, common_const_js_1.CommonConst.OBFUSCATION_TXT)));
      } catch (t) {
        this.logger.printErrorExit("COPY_FILE_ERROR", [e]);
      }
    }
  }
  resolveObfuscationFiles() {
    var e;
    const t = [];
    if (!this._harExtendInfo.isObfuscatedHar()) return t;
    return (null !== (e = this.moduleModel.getProfileOpt().buildOptionSet) && undefined !== e ? e : []).forEach(e => {
      var i, s, o, r, a;
      t.push(...(0, obfuscation_config_resolver_js_1.transformRules)(null === (o = null === (s = null === (i = e.arkOptions) || undefined === i ? undefined : i.obfuscation) || undefined === s ? undefined : s.ruleOptions) || undefined === o ? undefined : o.files, this._moduleDir, this.moduleModel, this.targetName)), t.push(...(0, obfuscation_config_resolver_js_1.transformRules)(null === (a = null === (r = e.arkOptions) || undefined === r ? undefined : r.obfuscation) || undefined === a ? undefined : a.consumerFiles, this._moduleDir, this.moduleModel, this.targetName));
    }), t;
  }
  noNeedCopyCmakeList(e) {
    return !(this.hasNativeOption && e === this.cmakeListDir);
  }
  copyLocalDepsFileToTempDir() {
    (0, npm_utils_js_1.collectLocalFileDependency)(this.packageManagerPath, this.packageJsonObj).forEach(e => {
      const t = path_1.resolve(this._moduleDir, e);
      fs.pathExistsSync(t) && fs.copySync(t, path_1.resolve(this.taskTmpDir, path_1.relative(this._moduleDir, t)));
    }), this.localProductDependenciesPath.length > 0 && (0, npm_utils_js_1.copyParameterizationLocalProductDependencies)({
      localProductDependencies: this.localProductDependenciesPath,
      moduleDir: this._moduleDir,
      tempDir: this.taskTmpDir,
      logger: this.logger
    });
  }
  getResourcesDirectories() {
    var e;
    const t = null === (e = this.targetService.getTargetData().getTargetOpt().resource) || undefined === e ? undefined : e.directories;
    return (null == t ? undefined : t.length) ? t : [path_1.relative(this.targetService.getTargetData().getPathInfo().getModulePath(), this.targetData.getModuleSourceSetModel().getTargetResPath())];
  }
  isResourcePath(e) {
    return this.getResourcesDirectories().some(t => e.includes(t));
  }
  copyResourceFileOfReleaseHar() {
    this.getResourcesDirectories().forEach(e => {
      const t = file_util_js_1.FileUtil.convertToAbsolutePath(e, this.targetService.getTargetData().getPathInfo().getModulePath());
      if (fs.existsSync(t)) {
        const i = file_util_js_1.FileUtil.convertToAbsolutePath(e, this.taskTmpDir);
        fs.copySync(t, i, {
          filter: e => !this.isResourceIgnored(e)
        });
      }
    });
  }
  copyModuleSourceFileToTempDirByWhiteList() {
    for (const e of this.releaseWhiteListTemDir) {
      const t = path_1.resolve(this._moduleDir, e);
      if (fs.existsSync(t)) {
        const i = path_1.resolve(this.taskTmpDir, e);
        fs.copySync(t, i, {
          overwrite: false
        });
      }
    }
  }
  getCopyFilter(e) {
    return t => this.noNeedCopyCmakeList(t) && !(this.isResourcePath(t) && this.isResourceIgnored(t)) && !this.needIgnoreSrc(e, t) && !this.excludeObfuscationFiles(t) || this.needCopyFile(t);
  }
  needCopyFile(e) {
    return -1 !== this.needCopyFileNames.indexOf(e);
  }
  isResourceIgnored(e) {
    const t = path_1.basename(e),
      i = this.ignoreResourcePattern;
    return !!i && i.some(e => (0, minimatch_1.minimatch)(t, e));
  }
  excludeObfuscationFiles(e) {
    return !!this.obfuscationFiles.find(t => "" === path_1.relative(e, t));
  }
  needIgnoreSrc(e, t) {
    const i = e.length,
      s = t.substring(i + 1).split(path_1.sep).join("/");
    return this.ohpmIgnoreRules.some(e => (0, minimatch_1.minimatch)(s, e) || (0, minimatch_1.minimatch)(`/${s}`, e));
  }
  copyModuleSourceFileToTempDirByBlackList() {
    fs.readdirSync(this._moduleDir).forEach(e => {
      const t = path_1.resolve(this._moduleDir, e);
      this.allInvalidPrefix.includes(t) || fs.copySync(t, path_1.resolve(this.taskTmpDir, e), {
        filter: this.getCopyFilter(this._moduleDir)
      });
    });
    const e = this.pathInfo.getHarGenerateBuildProfilePath(),
      t = path_1.resolve(this.taskTmpDir, build_directory_const_js_1.BuildArtifactConst.BUILD_PROFILE_FILE);
    fs.existsSync(e) && fs.copySync(e, t), fs.pathExistsSync(this.harModuleJson) && fs.removeSync(this.harModuleJson);
  }
  preCheckBeforePack() {
    const e = this.packageManagerPath,
      t = this.packageJsonObj;
    (0, npm_utils_js_1.checkHasLocalFileDependency)(e, t) && this.logger._buildError(`Local dependencies detected during har packing of module ${this.moduleName}. Declaring local dependencies in har module might cause failing during install har package.`)._detail("Check package.json/oh-package.json5 of current library module and replace the local dependencies.")._file(e)._printWarn(this.moduleName), fs.emptyDirSync(this.taskTmpDir);
  }
  copyBuildIntermediatesOutToTempDir() {
    fs.pathExistsSync(this.processedLibs) && fs.copySync(this.processedLibs, path_1.resolve(this.taskTmpDir, build_directory_const_js_1.BuildDirConst.LIBS)), fs.pathExistsSync(this.resourceTablePath) && fs.copySync(this.resourceTablePath, path_1.resolve(this.taskTmpDir, build_directory_const_js_1.BuildArtifactConst.RESOURCE_TABLE_TXT));
    const e = path_1.resolve(this.pathInfo.getIntermediatesResProfilePath(), build_directory_const_js_1.BuildArtifactConst.INSIGHT_INTENT_JSON);
    fs.pathExistsSync(e) && fs.pathExistsSync(this.getByteCodeHarInsightIntentDesTempPath()) && fs.copySync(e, this.getByteCodeHarInsightIntentDesTempPath()), fs.copySync(this.harProfile, path_1.resolve(this.taskTmpDir, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN));
  }
  getByteCodeHarInsightIntentDesTempPath() {
    const e = path_1.resolve(this.pathInfo.getModuleSrcMainResourceBaseProfilePath(), build_directory_const_js_1.BuildArtifactConst.INSIGHT_INTENT_JSON);
    return path_1.resolve(this.taskTmpDir, path_1.relative(this._moduleDir, e));
  }
  copyOtherGenerateFiles() {}
  processHarRouterMap() {}
  processHarResStartupConfig() {}
  processBundleDepRes() {}
  processObfuscatedHarAbility() {}
  copyIntermediatePacToTempDir() {}
}
__decorate([(0, hvigor_1.Input)()], AbstractProcessHarArtifacts.prototype, "ignoreResourcePattern", null);
