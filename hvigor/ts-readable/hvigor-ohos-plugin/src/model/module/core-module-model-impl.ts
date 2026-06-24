/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/module/core-module-model-impl.js
 */
import os_1 from "os";
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as fs from "fs-extra";
import fs_extra_1 from "fs-extra";
import * as ohos_trace_js_1 from "../../common/trace/ohos-trace";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as device_type_const_js_1 from "../../const/device-type-const";
import * as compile_mode_enum_js_1 from "../../enum/compile-mode-enum";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as hap_extra_info_js_1 from "../../project/data/hap-extra-info";
import * as array_util_js_1 from "../../utils/array-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as error_handlers_js_1 from "../../utils/validate/error-handlers";
import * as validate_util_js_1 from "../../utils/validate/validate-util";
import * as file_util_js_1 from "../../utils/file-util";
import * as module_build_profile_loader_js_1 from "../../utils/loader/file/module-build-profile-loader";
import * as oh_package_loader_js_1 from "../../utils/loader/file/oh-package-loader";
import * as res_model_loader_js_1 from "../../utils/loader/file/res-model-loader";
const log = ohos_logger_js_1.OhosLogger.getLogger("ohos-core-module-model"),
  invalidBuildProfilePath = new Set();
export class CoreModuleModelImpl {
  constructor(e, t) {
    this.compilePluginPath = undefined, this.targetSourceSetMap = new Map(), this.module = e, this.name = e.getName(), this.modulePath = e.getNodeDir(), this.parentProject = t, this.buildProfilePath = path_1.resolve(this.modulePath, common_const_js_1.CommonConst.PROFILE_JSON5), this.checkModuleBuildProfile(this.buildProfilePath), this.moduleBuildOpt = module_build_profile_loader_js_1.moduleBuildProfileLoader.getBuildProfile(this.name), this._sourceRootDir = path_1.resolve(this.modulePath, "src"), this.targets = this.getOrDefaultTargets(), this.belongProjectPath = this.initBelongProjectPath(this.modulePath), this.trace();
  }
  initBelongProjectPath(e) {
    return e.startsWith(this.parentProject.getProjectDir() + path_1.sep) ? this.parentProject.getProjectDir() : path_1.resolve(this.modulePath, "..");
  }
  findBelongProjectPath(e) {
    if (e === path_1.dirname(e)) return path_1.resolve(this.modulePath, "..");
    const t = path_1.resolve(e, common_const_js_1.CommonConst.PROFILE_JSON5);
    if (invalidBuildProfilePath.has(t)) return this.findBelongProjectPath(path_1.dirname(e));
    if (!fs_extra_1.existsSync(t)) return invalidBuildProfilePath.add(t), this.findBelongProjectPath(path_1.dirname(e));
    const o = hvigor_1.Json5Reader.getJson5Obj(t);
    return t && o.modules ? e : (invalidBuildProfilePath.add(t), this.findBelongProjectPath(path_1.dirname(e)));
  }
  getBelongProjectPath() {
    return this.belongProjectPath;
  }
  getCompilePluginPath() {
    return this.compilePluginPath;
  }
  setCompilePluginPath(e) {
    this.compilePluginPath = e;
  }
  refreshData() {
    this.moduleBuildOpt = module_build_profile_loader_js_1.moduleBuildProfileLoader.getBuildProfile(this.name);
    this.getOrDefaultTargets().forEach(e => {
      const t = this.targets.find(t => t.name === e.name);
      if (t) for (const o of Object.keys(e)) t[o] = e[o];
    }), this.trace();
  }
  trace() {
    ohos_trace_js_1.ohosTrace.traceModules(this.name, this.moduleBuildOpt.apiType);
  }
  isOhpmProject() {
    return this.parentProject.isOhpmProject();
  }
  isMixedDeviceModule() {
    return this.getDeviceTypes().some(e => device_type_const_js_1.DeviceTypeConst.RICH_DEVICES.includes(e)) && this.getDeviceTypes().some(e => device_type_const_js_1.DeviceTypeConst.LITE_DEVICES.includes(e));
  }
  isPurerRichDeviceModule() {
    return this.getDeviceTypes().every(e => device_type_const_js_1.DeviceTypeConst.RICH_DEVICES.includes(e));
  }
  haveRichDevicesInModule() {
    return this.getDeviceTypes().some(e => device_type_const_js_1.DeviceTypeConst.RICH_DEVICES.includes(e));
  }
  haveLiteDevicesInModule() {
    return this.getDeviceTypes().some(e => device_type_const_js_1.DeviceTypeConst.LITE_DEVICES.includes(e));
  }
  isPurerLiteDeviceModule() {
    return this.getDeviceTypes().every(e => device_type_const_js_1.DeviceTypeConst.LITE_DEVICES.includes(e));
  }
  isSingleDeviceTypeModule() {
    return this.isPurerRichDeviceModule() || this.isPurerLiteDeviceModule();
  }
  getRelatedEntryModules() {
    if (this.getModuleType() !== module_type_enum_js_1.ModuleType.Feature) return;
    const e = this.getProfileOpt().entryModules;
    return this.getApiType() === hap_extra_info_js_1.ApiType.FA && undefined === e && log.printErrorExit("NO_ENTRY_MODULES_IN_FA_PROJECT", [], [[this.getProfilePath(), this.getName()]]), null != e ? e : undefined;
  }
  getSourceRootByTargetName(e = common_const_js_1.DefaultTargetConst.DEFAULT_TARGET) {
    return this.targetSourceSetMap.has(e) ? this.targetSourceSetMap.get(e).getSourceSetRoot() : this.targetSourceSetMap.get(common_const_js_1.DefaultTargetConst.DEFAULT_TARGET).getSourceSetRoot();
  }
  getBuildProfileName() {
    return common_const_js_1.CommonConst.PROFILE_JSON5;
  }
  getName() {
    return this.name;
  }
  getProjectDir() {
    return this.modulePath;
  }
  getParentProject() {
    return this.parentProject;
  }
  getPackageJsonPath() {
    return path_1.resolve(this.modulePath, common_const_js_1.CommonConst.PACKAGE_JSON);
  }
  getOhPackageJson5Path() {
    return oh_package_loader_js_1.ohPackageLoader.getNodeOhPackagePath(path_1.resolve(this.modulePath, common_const_js_1.CommonConst.OH_PACKAGE_JSON5));
  }
  getProfilePath() {
    return this.buildProfilePath;
  }
  getProfileOpt() {
    return this.moduleBuildOpt;
  }
  isArkModule() {
    return true;
  }
  isHapModule() {
    return this.getModuleType() === module_type_enum_js_1.ModuleType.Entry || this.getModuleType() === module_type_enum_js_1.ModuleType.Feature;
  }
  isHarModule() {
    return this.getModuleType() === module_type_enum_js_1.ModuleType.Har;
  }
  isHspModule() {
    return this.getModuleType() === module_type_enum_js_1.ModuleType.Shared;
  }
  getProductApiMeta(e) {
    return this.getParentProject().getProductApiMeta(e);
  }
  getCompileApiMetaByProduct(e) {
    const t = this.getProductApiMeta(e);
    if (!t) throw new Error(`Can not find product ${e}.`);
    return t.compileSdkVersion;
  }
  getCompatibleApiMetaByProduct(e) {
    const t = this.getProductApiMeta(e);
    if (!t) throw new Error(`Can not find product ${e}.`);
    return t.compatibleSdkVersion;
  }
  getOriginCompatibleSdkVersionByProduct(e) {
    const t = this.getProductApiMeta(e);
    return t || log.printErrorExit("PRODUCT_NAME_NOT_FOUND_IN_PROFILE", [e, this.buildProfilePath]), t.originCompatibleSdkVersion;
  }
  getTargetApiMetaByProduct(e) {
    var t;
    return null === (t = this.getProductApiMeta(e)) || undefined === t ? undefined : t.targetSdkVersion;
  }
  getAllModules() {
    return this.parentProject.getAllModules();
  }
  getApiType() {
    return undefined === this.moduleBuildOpt.apiType ? hap_extra_info_js_1.ApiType.STAGE : this.moduleBuildOpt.apiType;
  }
  getCompileMode(e) {
    const t = this.getCompileApiMetaByProduct(e.name).version;
    return this.getApiType() === hap_extra_info_js_1.ApiType.FA || t < 9 ? compile_mode_enum_js_1.CompileModeEnum.JS_BUNDLE : compile_mode_enum_js_1.CompileModeEnum.ES_MODULE;
  }
  getTargetOptions() {
    return this.targets;
  }
  getOrDefaultTargets() {
    const e = this.getProfileOpt().targets;
    if (undefined === e) return [{
      name: "default"
    }];
    return e.map(e => e.name).includes("default") || e.push({
      name: "default"
    }), e;
  }
  moduleBuildProfileCheck(e) {
    const t = e ? common_const_js_1.BuildProfileSchemaFileConst.HAP_MODULE_BUILD_PROFILE_SCHEMA_PATH : common_const_js_1.BuildProfileSchemaFileConst.HAR_MODULE_BUILD_PROFILE_SCHEMA_PATH;
    validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.name,
      filePath: this.buildProfilePath,
      schemaPath: t,
      errorHandlers: [error_handlers_js_1.handleEntryModulesErrors]
    }), validate_util_js_1.ValidateUtil.validateDuplicatedName(this.getProfileOpt().targets, "targets", this.buildProfilePath), this.warnIfUseAbsolutePath(), this.validateCompileMode();
  }
  warnIfUseAbsolutePath() {
    var e;
    if (this.getProfileOpt().targets && this.getProfileOpt().targets.length > 0) for (const t of this.getProfileOpt().targets) if ((null === (e = t.resource) || undefined === e ? undefined : e.directories) && t.resource.directories.length > 0) for (const e of t.resource.directories) path_1.isAbsolute(e) && log.warn(`In the build-profile.json5 file of module '${this.name}', the absolute path '${e}' is configured under targets -> resource -> directories. The path for 'directories' must start with "./". At file: ${this.buildProfilePath}`);
  }
  checkModuleBuildProfile(e) {
    fs.existsSync(e) || log.printErrorExit("BUILD_CONFIG_FILE_NOT_FOUND", [this.name, e], [[e]]);
  }
  validateCompileMode() {
    var e;
    const t = hvigor_1.ParseJsonFile.parseJsonFile(this.buildProfilePath, true),
      o = null === (e = null == t ? undefined : t.buildOption) || undefined === e ? undefined : e.compileMode;
    if (o) {
      const e = o._line,
        t = o._column + 1;
      log.warn(`compileMode has been deprecated. The value of it is invalid.${os_1.EOL}         at ${this.buildProfilePath}:${e}:${t}`);
    }
  }
  getModule() {
    return this.module;
  }
  processTargetsConfig() {
    const e = (0, array_util_js_1.getElementFromArr)(this.moduleBuildOpt.targets, "default"),
      t = (0, array_util_js_1.getElementFromArr)(this.moduleBuildOpt.targets, "ohosTest");
    undefined !== t && undefined === t.runtimeOS && (t.runtimeOS = null == e ? undefined : e.runtimeOS);
  }
  getWorkerConfig(e) {
    var t, o;
    return null === (o = null === (t = null == e ? undefined : e.sourceOption) || undefined === t ? undefined : t.workers) || undefined === o ? undefined : o.map(e => file_util_js_1.FileUtil.convertToAbsolutePath(e, this.getProjectDir()));
  }
  getAllTargetSourceSet() {
    return this.targetSourceSetMap;
  }
  getJsonProfilePath(e, t = false) {
    const o = t ? common_const_js_1.CommonConst.CONFIG_JSON : common_const_js_1.CommonConst.MODULE_JSON5,
      r = this.getSourceSetByTargetName(e).getSourceSetRoot();
    return path_1.resolve(r, o);
  }
  getJsonProfileOptByTargetName(e, t = false) {
    const o = this.getJsonProfilePath(e, t);
    return t ? res_model_loader_js_1.resModelLoader.getConfigJson(o) : res_model_loader_js_1.resModelLoader.getModuleJson(o);
  }
  getJsonProfileByModel(e, t) {
    const o = this.getJsonProfileOptByTargetName(t, e.isFaMode()),
      r = this.getJsonProfilePath(t, e.isFaMode());
    return hvigor_1.globalData.cliOpts._.includes(common_const_js_1.CommonConst.ON_DEVICE_TEST) && !o && log.printErrorExit("ON_DEVICE_TEST_FAIL", [r, common_const_js_1.CommonConst.ON_DEVICE_TEST], [[r]]), {
      jsonFilePath: r,
      profile: o,
      deviceTypes: e.isFaMode() ? o.module.deviceType : o.module.deviceTypes,
      deviceConfig: e.isFaMode() ? common_const_js_1.CommonConst.DEVICE_TYPE : common_const_js_1.CommonConst.DEVICE_TYPES,
      configurationProfile: e.isFaMode() ? common_const_js_1.CommonConst.CONFIG_JSON : common_const_js_1.CommonConst.MODULE_JSON5
    };
  }
  isCrossplatformModule(e) {
    var t;
    return !!e && e.crossplatform && (null === (t = e.modules) || undefined === t ? undefined : t.includes(this.getModule().getName()));
  }
  isIgnoreCrossPlatformCheckModule(e) {
    var t;
    return !!(null === (t = null == e ? undefined : e.buildOption) || undefined === t ? undefined : t.ignoreCrossPlatform);
  }
  getRemoteHspPath() {
    return path_1.resolve(this.getProjectDir(), common_const_js_1.CommonConst.OH_MODULES, build_directory_const_js_1.BuildArtifactExtension.DOT_HSP);
  }
  getMockConfigPath() {
    return path_1.join(this.modulePath, "src", "mock", common_const_js_1.CommonConst.MOCK_CONFIG_JSON5);
  }
  isOutModule() {
    return this.parentProject.getProjectDir() !== this.belongProjectPath;
  }
}
