/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hsp/prepare-shared-har-res.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as fs from "fs-extra";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as cmake_util_js_1 from "../../utils/cmake/cmake-util";
import * as dependency_util_js_1 from "../../utils/dependency-util";
import * as file_util_js_1 from "../../utils/file-util";
import * as oh_package_loader_js_1 from "../../utils/loader/file/oh-package-loader";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as npm_utils_js_1 from "../../utils/npm-utils";
import * as oh_exports_util_js_1 from "../../utils/oh-exports-util";
import * as parameter_utils_js_1 from "../../utils/parameter-utils";
import * as dependency_module_version_validate_js_1 from "../../validate/dependency-module-version-validate";
import * as task_names_js_1 from "../common/task-names";
import * as har_extend_info_js_1 from "../har/har-extend-info";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
var __decorate = this && this.__decorate || function (e, t, s, i) {
  var a,
    o = arguments.length,
    r = o < 3 ? t : null === i ? i = Object.getOwnPropertyDescriptor(t, s) : i;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) r = Reflect.decorate(e, t, s, i);else for (var n = e.length - 1; n >= 0; n--) (a = e[n]) && (r = (o < 3 ? a(r) : o > 3 ? a(t, s, r) : a(t, s)) || r);
  return o > 3 && r && Object.defineProperty(t, s, r), r;
};
var Task = task_names_js_1.TaskNames.Task;
const _log = ohos_logger_js_1.OhosLogger.getLogger("PrepareSharedHarRes");
export class PrepareSharedHarRes extends ohos_hap_task_js_1.OhosHapTask {
  declareInputs() {
    var e;
    const t = new Map();
    return this.packageJsonObj && t.set("modulePackageJsonObj", JSON.stringify(this.packageJsonObj)), t.set("integratedHsp", !!(null === (e = this.targetService.getBuildOption().arkOptions) || undefined === e ? undefined : e.integratedHsp)), t.set(common_const_js_1.CommonConst.KEEP_DEPENDENCY, !!(0, parameter_utils_js_1.getPropertyKeepDependency)()), t;
  }
  declareInputFiles() {
    const e = this.allInvalidPrefix.join("|").replace(/\\/g, "\\\\"),
      t = path_1.resolve(this.module.getBuildProfilePath()).replace(/\\/g, "\\\\"),
      s = RegExp(`^(?!${e}).*|${t}`),
      i = super.declareInputFiles().addEntry(this.inputPackageJson5Path).addEntry(this.inputModuleJsonPath).addEntry(this._moduleDir, {
        isDirectory: true,
        test: s
      }).addEntry(this.libsPath);
    fs_extra_1.existsSync(this.inputDeclareFilesPath) && i.addEntry(this.inputDeclareFilesPath, {
      isDirectory: true
    }), fs_extra_1.existsSync(this.inputResourceStr) && i.addEntry(this.inputResourceStr), fs_extra_1.existsSync(this.inputPacJsonFilePath) && i.addEntry(this.inputPacJsonFilePath);
    for (const e of this.headerPath) {
      const t = path_1.isAbsolute(e) ? e : path_1.resolve(this._moduleDir, e);
      fs.pathExistsSync(t) && i.addEntry(t);
    }
    return i;
  }
  declareOutputFiles() {
    return super.declareOutputFiles().addEntry(this.taskTmpDir, {
      isDirectory: true
    });
  }
  constructor(e) {
    var t;
    super(e, Task.PREPARE_SHARED_HAR_RES), this.INTERFACE_HAR = "InterfaceHar", this.ignoreFileList = [build_directory_const_js_1.BuildDirConst.LIBS, build_directory_const_js_1.BuildDirConst.BUILD_ROOT, common_const_js_1.CommonNodeConst.NODE_MODULES, common_const_js_1.CommonConst.OH_MODULES, ".cxx", ".previewer", ".hvigor", ".gitignore", ".ohpmignore"], this.allInvalidPrefix = [];
    const s = this.targetService.getBuildOption();
    this.inputPackageJson5Path = e.getTargetData().getModuleModel().getOhPackageJson5Path(), this.inputModuleJsonPath = this.pathInfo.getIntermediatesProcessProfile(), this.inputResourceStr = this.pathInfo.getResourceStr(), this.inputDeclareFilesPath = this.pathInfo.getIntermediatesEtsTgzPath(this.moduleName), this.inputPacJsonFilePath = this.pathInfo.getIntermediatesPacJsonPath();
    const i = null === (t = s.nativeLib) || undefined === t ? undefined : t.headerPath;
    "string" == typeof i ? this.headerPath = [i] : Array.isArray(i) ? this.headerPath = i : this.headerPath = [], this.libsPath = this.pathInfo.getIntermediatesStrippedLibsDir(), this.taskTmpDir = this.getTaskTempDir(this.targetData, "shared_har", false), this.outputPackageJsonPath = oh_package_loader_js_1.ohPackageLoader.getNodeOhPackagePath(path_1.resolve(this.taskTmpDir, common_const_js_1.CommonConst.OH_PACKAGE_JSON5)), this.outputModuleJsonPath = path_1.resolve(this.taskTmpDir, "src", "main", common_const_js_1.CommonConst.MODULE_JSON), this.outputResourceStr = path_1.resolve(this.taskTmpDir, "src", "main", "resource"), this.outputDeclareFilesPath = this.taskTmpDir, this.outputPacJsonFilePath = path_1.resolve(this.taskTmpDir, common_const_js_1.CommonConst.PAC_JSON), this._moduleDir = this.moduleModel.getProjectDir(), this.initAllInvalidPrefix(), this._harExtendInfo = new har_extend_info_js_1.HarExtendInfo(this.moduleModel, e.getBuildOption());
    const a = cmake_util_js_1.CmakeUtil.nativeTaskCondition(this.moduleModel, this.targetData, s.externalNativeOptions);
    this.hasNativeOption = a, this.cmakeListDir = a ? cmake_util_js_1.CmakeUtil.getCmakeListDir(this.moduleModel, this.targetName, s.externalNativeOptions) : undefined;
  }
  initTaskDepends() {
    this.targetName === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET ? this.declareDepends("OhosTestCompileArkTS") : this.declareDepends("CompileArkTS"), this.declareDepends(task_names_js_1.TaskNames.Task.DO_NATIVE_STRIP.name);
  }
  async doTaskAction() {
    this.preCheckBeforePack(), this.copyLocalDepsFileToTempDir(), file_util_js_1.FileUtil.copySpecialFileToTempDir(this.moduleModel.getProjectDir(), this.taskTmpDir), fs_extra_1.existsSync(this.inputDeclareFilesPath) && fs_extra_1.copySync(this.inputDeclareFilesPath, this.outputDeclareFilesPath), this.copyIntermediatePacToTempDir(), await this.generatePackageJson(), this.generateModuleJson(), this.copyResourcesTxt(), this.initRes(), await this.copyObfuscationRules(), await this.syncNative();
  }
  copyIntermediatePacToTempDir() {
    fs_extra_1.existsSync(this.inputPacJsonFilePath) && fs_extra_1.copySync(this.inputPacJsonFilePath, this.outputPacJsonFilePath);
  }
  get obfuscationRulesPath() {
    return this.targetService.getTargetData().getPathInfo().getObfuscationExportRulePath();
  }
  async copyObfuscationRules() {
    fs_extra_1.existsSync(this.obfuscationRulesPath) && (await fs_extra_1.copy(this.obfuscationRulesPath, path_1.join(this.taskTmpDir, common_const_js_1.CommonConst.OBFUSCATION_TXT)));
  }
  copyResourcesTxt() {
    const e = path_1.resolve(this.pathInfo.getIntermediatesRes(), build_directory_const_js_1.BuildArtifactConst.RESOURCE_TABLE_TXT);
    fs_extra_1.existsSync(e) && fs_extra_1.copySync(e, path_1.resolve(this.taskTmpDir, build_directory_const_js_1.BuildArtifactConst.RESOURCE_TABLE_TXT));
  }
  async syncNative() {
    for (const e of this.headerPath) {
      const t = new Map(),
        s = path_1.isAbsolute(e) ? e : path_1.resolve(this._moduleDir, e);
      cmake_util_js_1.CmakeUtil.checkNativeHeader(s, this.moduleModel, t), fs_extra_1.existsSync(s) && (await fs_extra_1.copy(s, path_1.resolve(this.taskTmpDir, "include"), {
        recursive: true
      }));
    }
    fs_extra_1.existsSync(this.libsPath) && fs_extra_1.readdirSync(this.libsPath).length > 0 && !this.excludeSoFromInterfaceHar() && (await fs_extra_1.copy(this.libsPath, path_1.resolve(this.taskTmpDir, "libs"), {
      recursive: true
    }));
  }
  async generatePackageJson() {
    var e, t, s, i;
    let a = this.packageJsonObj;
    a.types = (0, npm_utils_js_1.getTypesFieldFromMainField)(a.main), a.main = undefined, a.packageType && a.packageType !== this.INTERFACE_HAR && _log.warn("share library packageType in oh-package.json should use InterfaceHar"), a.packageType || (a.packageType = this.INTERFACE_HAR);
    const o = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.strictMode) || undefined === t ? undefined : t.useNormalizedOHMUrl,
      r = path_1.relative(this.pathInfo.getModuleBuildPath(), this.pathInfo.getGenerateBuildProfileDir()),
      n = [...((null === (s = this.targetData.getTargetOpt().source) || undefined === s ? undefined : s.sourceRoots) || []), `./${build_directory_const_js_1.BuildDirConst.SRC}/${build_directory_const_js_1.BuildDirConst.MAIN}`, `./${this.pathInfo.getBuildRoot()}/${r.replace(/\\/g, "/")}/${this.targetName}`],
      l = {
        useNormalizedOHMUrl: o,
        integratedHsp: null === (i = this.targetService.getBuildOption().arkOptions) || undefined === i ? undefined : i.integratedHsp,
        sourceRoots: n,
        debug: this.targetService.isDebug()
      };
    a.metadata ? Object.assign(a.metadata, l) : Object.assign(a, {
      metadata: l
    }), (0, dependency_util_js_1.minimizeHspDependencies)(a, this.service, this.packageManagerPath);
    let u = new Set();
    const d = this.pathInfo.getIntermediatesProcessLibs();
    fs.existsSync(d) && (u = await file_util_js_1.FileUtil.readDirFileNames(d));
    const c = cmake_util_js_1.CmakeUtil.getHarExtraParams(this);
    a = file_util_js_1.FileUtil.mergeExtraParamsToOhPack(a, c, this.targetName, u), (0, dependency_module_version_validate_js_1.resolveJsonObjWithoutAtModule)(a, this.projectModel.getProjectDir()), a[common_const_js_1.CommonConst.OH_EXPORTS] && (a[common_const_js_1.CommonConst.OH_EXPORTS] = (0, oh_exports_util_js_1.rewriteOhExports)(this.moduleModel.getProjectDir(), this.outputDeclareFilesPath, a[common_const_js_1.CommonConst.OH_EXPORTS])), fs_extra_1.outputJsonSync(this.outputPackageJsonPath, a, {
      encoding: "utf8",
      spaces: "  "
    });
  }
  copyLocalDepsFileToTempDir() {
    (0, npm_utils_js_1.collectLocalFileDependency)(this.inputPackageJson5Path, this.packageJsonObj).forEach(e => {
      const t = path_1.resolve(this._moduleDir, e);
      fs_extra_1.pathExistsSync(t) && fs.copySync(t, path_1.resolve(this.taskTmpDir, path_1.relative(this._moduleDir, t)));
    }), this.localProductDependenciesPath.length > 0 && (0, npm_utils_js_1.copyParameterizationLocalProductDependencies)({
      localProductDependencies: this.localProductDependenciesPath,
      moduleDir: this._moduleDir,
      tempDir: this.taskTmpDir,
      logger: _log
    });
  }
  generateModuleJson() {
    var e;
    const t = fs_extra_1.readJsonSync(this.inputModuleJsonPath, {
      encoding: "utf8"
    });
    let s = t.app.bundleName;
    const i = null === (e = this.targetService.getBuildOption().arkOptions) || undefined === e ? undefined : e.integratedHsp;
    this.moduleModel.isHspModule() && i && (s = ""), t.app = {
      bundleName: s,
      bundleType: t.app.bundleType,
      debug: t.app.debug,
      versionCode: t.app.versionCode,
      versionName: t.app.versionName,
      minCompatibleVersionCode: t.app.minCompatibleVersionCode,
      minAPIVersion: t.app.minAPIVersion,
      targetAPIVersion: t.app.targetAPIVersion,
      apiReleaseType: t.app.apiReleaseType,
      targetMinorAPIVersion: t.app.targetMinorAPIVersion,
      targetPatchAPIVersion: t.app.targetPatchAPIVersion
    }, t.module = {
      name: t.module.name,
      packageName: t.module.packageName,
      type: t.module.type,
      deliveryWithInstall: t.module.deliveryWithInstall,
      deviceTypes: t.module.deviceTypes,
      requestPermissions: t.module.requestPermissions,
      dependencies: t.module.dependencies,
      libIsolation: t.module.libIsolation
    }, fs_extra_1.outputJsonSync(this.outputModuleJsonPath, t, {
      encoding: "utf8"
    });
  }
  preCheckBeforePack() {
    const e = this.moduleModel.getOhPackageJson5Path(),
      t = this.packageJsonObj;
    return (0, npm_utils_js_1.checkHasLocalFileDependency)(e, t) && _log._buildError(`Local dependencies detected during har packing of module ${this.moduleName}. Declaring local dependencies in har module might cause failing during install har package.`)._detail("Check oh-package.json5 file of current shared library module and replace the local dependencies.")._file(e)._printWarn(this.moduleName), fs_extra_1.emptyDirSync(this.taskTmpDir), true;
  }
  initAllInvalidPrefix() {
    for (const e of this.ignoreFileList) this.allInvalidPrefix.push(path_1.resolve(this._moduleDir, e));
  }
  initRes() {
    fs_extra_1.existsSync(this.pathInfo.getResourceStr()) && fs_extra_1.copySync(this.inputResourceStr, this.outputResourceStr);
  }
  excludeSoFromInterfaceHar() {
    var e, t, s;
    const i = this.targetService.getBuildOption(),
      a = null === (e = i.nativeLib) || undefined === e ? undefined : e.headerPath,
      o = null !== (s = null === (t = i.nativeLib) || undefined === t ? undefined : t.excludeSoFromInterfaceHar) && undefined !== s && s;
    return a && o ? (_log.warn(`When headerPath is set, .so and header files must be included in the interface HAR for C++ API exposure and .so linking. \n              excludeSoFromInterfaceHar: true has no effect in this case.\n              Check the headerPath and excludeSoFromInterfaceHar configuration in ${path_1.resolve(this.pathInfo.getModulePath(), "build-profile.json5")}.`), false) : o;
  }
}
__decorate([(0, hvigor_1.InputFile)()], PrepareSharedHarRes.prototype, "obfuscationRulesPath", null);
