/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract/abstract-pre-build.js
 */
import * as fs from "fs";
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as json_reader_1 from "@ohos/hvigor/src/base/util/json-reader";
import * as node_config_path_info_1 from "@ohos/hvigor/src/common/node-config-path-info";
import fs_extra_1 from "fs-extra";
import * as build_option_path_info_js_1 from "../../common/build-option-path-info";
import * as common_util_js_1 from "../../common/common-util";
import * as project_ohos_config_manager_js_1 from "../../common/global/project-ohos-config-manager";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as device_type_const_js_1 from "../../const/device-type-const";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as code_type_enum_js_1 from "../../enum/code-type-enum";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as runtime_type_enum_js_1 from "../../enum/runtime-type-enum";
import * as hap_extra_info_js_1 from "../../project/data/hap-extra-info";
import * as project_extra_info_service_js_1 from "../../project/project-extra-info-service";
import * as file_util_js_1 from "../../utils/file-util";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as json_util_js_1 from "../../utils/json-util";
import * as oh_package_loader_js_1 from "../../utils/loader/file/oh-package-loader";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as validate_util_js_1 from "../../utils/validate/validate-util";
import * as abstract_merge_profile_js_1 from "../abstract-merge-profile";
import * as har_extend_info_js_1 from "../har/har-extend-info";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
import * as build_profile_utils_js_1 from "../../utils/build-profile-utils";
import * as find_target_product_js_1 from "../../common/find-target-product";
import * as global_project_data_service_js_1 from "../service/global-project-data-service";
import * as ohos_trace_js_1 from "../../common/trace/ohos-trace";
export class AbstractPreBuild extends ohos_hap_task_js_1.OhosHapTask {
  declareInputs() {
    var e, t;
    const i = this.sdkInfo.getToolchainsComponentVersion(),
      o = new Map();
    o.set("compileApiVersion", this.compileApiVersion).set("compatibleApiVersion", this.compatibleApiVersion).set("targetStatusCode", this.targetStatusCode).set("apiType", this.apiType).set("deviceType", this.deviceTypes).set("codeType", this.containsEts).set("sdkToolchainsComponentVersion", i).set("profileModuleName", this.moduleName).set(abstract_merge_profile_js_1.INPUT_KEY.moduleJsonOpt, JSON.stringify(this.jsonProfile)).set("isSupportOhpmProj", this.supportOHPM()).set("removePermissions", JSON.stringify(null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.removePermissions)).set("isPreloadSystemSoEnabled", this.isPreloadSystemSoEnabled).set("isSingleFileEmitEnabled", inject_util_js_1.InjectUtil.isSingleFileEmitEnabled()), undefined !== this.compilePluginFile && o.set("compilePluginFile", this.compilePluginFile);
    const s = this.targetService.getCustomTypes();
    if (o.set("customTypes", null != s ? s : ""), s && Array.isArray(s) && s.length > 0) for (const e of s) {
      if (this.dependencies[e] || this.dependencies[`@types/${e}`]) continue;
      const t = (0, build_profile_utils_js_1.getCustomTypePathIfExists)(this.moduleModel.getProjectDir(), e);
      o.set(e, !!t);
    }
    return o.set(common_const_js_1.CommonConst.REQUIRED_DEVICE_TYPE, null !== (t = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.REQUIRED_DEVICE_TYPE)) && undefined !== t ? t : ""), o;
  }
  declareInputFiles() {
    const e = super.declareInputFiles();
    return fs_extra_1.existsSync(this.jsonProfile.jsonFilePath) && e.addEntry(this.jsonProfile.jsonFilePath), this.fromConfigFilePathArr.forEach(t => {
      e.addEntry(t);
    }), e;
  }
  async beforeAlwaysAction() {
    await super.beforeAlwaysAction(), this.checkEntryModules(), this.checkEmptyProperty(), this.checkEmptyTargetSdkVersion(), ohos_trace_js_1.ohosTrace.traceIncludeInBuild(this.moduleName, true), ohos_trace_js_1.ohosTrace.traceModuleType(this.moduleName, this.moduleModel.getModuleType()), ohos_trace_js_1.ohosTrace.traceBuildOption(this.moduleName, this.targetService.getBuildOption()), ohos_trace_js_1.ohosTrace.tracePackageJson(this.moduleName, this.packageJsonObj);
  }
  checkEmptyTargetSdkVersion() {
    const e = global_project_data_service_js_1.GlobalProjectDataService.getInstance();
    if (undefined !== this.targetData.getTargetSdkVersion() || e.getIsEmptyTargetSdkVersionCheck()) return;
    e.setIsEmptyTargetSdkVersionCheck();
    const t = (0, hvigor_common_1.getOsLanguage)(),
      i = (0, hvigor_logger_1.getUrl)(hvigor_logger_1.DEVELOPER_URL),
      o = "The project has not explicitly set the 'targetSdkVersion' version at build-profile.json5. It is recommended to configure it. ";
    i ? this._log.warn(`${o}Reference: ${i}/consumer/${t}/doc/harmonyos-guides/ide-hvigor-build-profile-app#section45865492619`) : this._log.warn(`${o}Reference: Go to the official website to see Guides > Build Process > Configuration Files > Project-level build-profile.json5 File`);
  }
  constructor(e, t) {
    var i, o;
    super(e, t), this._log = ohos_logger_js_1.OhosLogger.getLogger(AbstractPreBuild.name), this.apiType = this.moduleModel.getApiType(), this.targetStatusCode = this.targetService.getTargetData().getTargetStatus().getStatusCode(), this.syscapJsonPath = path_1.resolve(null === (i = this.moduleModel) || undefined === i ? undefined : i.getSourceRootByTargetName(this.targetName), common_const_js_1.CommonConst.SYSCAP_JSON), this.containsEts = this.moduleModel.getSourceSetByTargetName(this.targetData.getTargetName()).getCodeMap().has(code_type_enum_js_1.CodeType.ETS), this.deviceTypes = this.targetData.getTargetDeviceType(), this.dependencies = this.getDependencies(this.moduleModel, true), this.compilePluginFile = null === (o = this.targetService.getBuildOption().arkOptions) || undefined === o ? undefined : o.compilePluginFile, this.dynamicDependencies = this.getModuleDynamicDependencies(), this.jsonProfile = this.moduleModel.getJsonProfileByModel(this.service, this.targetName), this.fromConfigFilePathArr = this.getFromConfigFilePathArr(), this.entryCardPath = path_1.resolve(this.moduleModel.getParentProject().getProjectDir(), "EntryCard"), this.projectProfilePath = this.projectModel.getProfilePath(), this.profilePath = this.moduleModel.getProfilePath();
  }
  initTaskDepends() {}
  async doTaskAction() {
    if (this.checkOutModule(), this.isPackingTestPackage()) throw (0, common_util_js_1.isSingleFramework)() ? new Error("API version 9 or later and Lite-Device projects do not support compilation test packages.") : new Error("API version 8 or later and Lite-Device projects do not support compilation test packages.");
    this.ohPackageCheck(), this.versionCheck(), this.checkCodeTypeForWearable(), this.checkCodeTypeForLiteWearable(), this.checkOhpmProjectSdkVersion(), this.doValidateForDiffApiType(), this.checkApiVersionForUnitTest(), this.checkRequestPermissions(), this.checkRemoveRequestPermissions(), this.validateAtomicService(), this.checkWidget(), this._log._printDebugCommand(`${process.platform}: JAVA_HOME, CLASSPATH`, [{
      JAVA_HOME: process.env.JAVA_HOME
    }, {
      CLASSPATH: process.env.CLASSPATH
    }]), this._log._printDebugCommand(`${process.platform}: NODE_HOME`, [{
      NODE_HOME: process.env.NODE_HOME
    }]), this.checkCustomTypesSupport(), this.checkHvigorPluginFile(), this.checkDynamicDependenciesSupport(), this.checkWorkerConfig(), this.checkExtendSourceDirs(), await this.checkPackageJson5Config(), this.checkDeviceTypes(), this.checkPathChinese(), this.checkByteCodeHar(), this.checkExcludeSoFromInterfaceHar(), this.validateFastBuildAppOnFAMode(), this.validatePreloadSystemSo(), this.checkExpandImportConfigs(), this.checkFormModule(), this.validateHarDeduplicate(), this.checkCustomizedHar();
  }
  checkOutModule() {
    const e = this.projectModel.isFaMode(),
      t = this.moduleModel.isOutModule();
    e && t && this._log.printErrorExit("THIS_PROJECT_IS_IN_THE_FA_MODEL_AND_DOES_NOT_ALLOW_FOR_EXTERNAL_DEPENDENCIES"), !e && this.moduleModel.getApiType() === hap_extra_info_js_1.ApiType.FA && this._log.printErrorExit("THIS_PROJECT_IS_IN_THE_STAGE_MODEL_AND_DOES_NOT_ALLOW_FOR_DEPENDENCIES_OF_MODULES_INTHE_FA_MODEL"), t && this.moduleModel.isHapModule() && this._log.printErrorExit("CANNOT_DEPEND_ON_HAP_MODULES_OUTSIDE_OF_THIS_PROJECT");
  }
  checkPathChinese() {
    this.moduleModelNodePaths.forEach(e => {
      /[\u4e00-\u9fff]/.test(e) && this._log.printErrorExit("PATH_CAN_NOT_CONTAIN_CHINESE_CHARACTERS", [e]);
    });
  }
  checkDeviceTypes() {
    const e = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.REQUIRED_DEVICE_TYPE);
    if (undefined === e || this.moduleModel.isHarModule()) return;
    const t = this.moduleModel.getDeviceTypes();
    (new Map([["phone", ["phone", "default"]], ["default", ["phone", "default"]], ["tablet", ["tablet", "2in1"]], ["2in1", ["tablet", "2in1"]]]).get(e) || [e]).some(e => t.includes(e)) || this._log.printErrorExit("THE_TYPE_OF_TARGET_DEVICEC_DOES_NOT_MATCH_THE_DEVICE_TYPE_CONFIGURED_BY_MODULE", [this.moduleModel.getName(), e, t.join()]);
  }
  checkHvigorPluginFile() {
    const e = this.compilePluginFile;
    if (undefined === e) return;
    this.moduleModel.isHarModule() && !new har_extend_info_js_1.HarExtendInfo(this.moduleModel, this.targetService.getBuildOption()).isObfuscatedHar() && this._log._buildError("'compilePluginFile' won't work for an unobfuscated library.")._file(this.moduleModel.getProfilePath())._printWarn(this.moduleName), e.startsWith(`./${build_directory_const_js_1.BuildDirConst.HVIGOR_COMPILE}/`) || this._log._buildError(`'compilePluginFile' must start with './${build_directory_const_js_1.BuildDirConst.HVIGOR_COMPILE}/'.`)._file(this.moduleModel.getProfilePath())._printErrorAndExit();
    const t = path_1.resolve(this.moduleModel.getProjectDir(), build_directory_const_js_1.BuildDirConst.HVIGOR_COMPILE);
    let i = path_1.resolve(this.moduleModel.getProjectDir(), e);
    (0, hvigor_common_1.isSubPath)(i, t) || this._log._buildError(`'compilePluginFile' is not under directory: '${t}'.`)._file(this.moduleModel.getProfilePath())._printErrorAndExit(), this.checkSuffix(i), fs_extra_1.existsSync(i) || (i = `${i}.ts`, fs_extra_1.existsSync(i) || this._log._buildError("'compilePluginFile' does not exist.")._file(this.moduleModel.getProfilePath())._printErrorAndExit());
  }
  checkSuffix(e) {
    const t = e.split(path_1.sep),
      i = t.length;
    if (0 === i) return true;
    const o = t[i - 1],
      s = o.lastIndexOf(".");
    if (s > 0) {
      ".ts" !== o.substring(s) && this._log._buildError("'compilePluginFile' must end with .ts.")._file(this.moduleModel.getProfilePath())._printErrorAndExit();
    }
    return true;
  }
  isPackingTestPackage() {
    return this.targetName === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET && this.compileApiVersion >= sdk_const_js_1.ApiVersion.API_VERSION_8 && this.service.hasLiteDevice();
  }
  validatePreloadSystemSo() {
    this.isPreloadSystemSoEnabled && inject_util_js_1.InjectUtil.isSingleFileEmitEnabled() && this._log.printErrorExit("SINGLE_EMIT_DOES_NOT_SUPPORT_WHEN_PRELOAD_SYSTEM_SO_ENABLED", [], [[path_1.resolve(hvigor_1.HVIGOR_PROJECT_WRAPPER_HOME, hvigor_1.DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME)]]);
  }
  supportOHPM() {
    return this.sdkInfo.supportOhpmProject(this.targetData.isHarmonyOS());
  }
  async checkPackageJson5Config() {
    if (this.moduleModel.getModuleType() !== module_type_enum_js_1.ModuleType.Har && this.moduleModel.getModuleType() !== module_type_enum_js_1.ModuleType.Shared) return;
    const e = await hvigor_1.Json5Reader.readJson5File(this.packageManagerPath),
      t = this.moduleModel.getProjectDir();
    if (this.checkMainAndTypesFile(e.main, e.types), e.main && !fs.existsSync(path_1.resolve(t, e.main)) && this._log.printErrorExit("MAIN_FILE_DOES_NOT_EXIST", [e.main, this.moduleModel.getOhPackageJson5Path()], [[e.main]]), e.types && !fs.existsSync(path_1.resolve(t, e.types)) && this._log.warn(`Invalid types file ${e.types} defined in ${this.moduleModel.getName()} ${common_const_js_1.CommonConst.OH_PACKAGE_JSON5}.`), e.main) {
      const i = path_1.resolve(t, e.main);
      fs.existsSync(i) && !i.includes(t) && this._log.warn(`Do not configure cross-module file ${e.main} defined in ${this.moduleModel.getName()} ${common_const_js_1.CommonConst.OH_PACKAGE_JSON5}.`);
    }
    if (e.types) {
      const i = path_1.resolve(t, e.types);
      fs.existsSync(i) && !i.includes(t) && this._log.warn(`Do not configure cross-module file ${e.types} defined in ${this.moduleModel.getName()} ${common_const_js_1.CommonConst.OH_PACKAGE_JSON5}.`);
    }
  }
  checkMainAndTypesFile(e, t) {
    if (e || t || this._log.warn(`Set either main or types, or both for this HSP/HAR module. at ${this.moduleModel.getName()} ${common_const_js_1.CommonConst.OH_PACKAGE_JSON5}.`), e) {
      const t = file_util_js_1.FileUtil.getFileSuffix(e);
      ("ets" !== t && "ts" !== t && "js" !== t || e.endsWith(".d.ets") || e.endsWith(".d.ts")) && this._log.printErrorExit("INVALID_MAIN_FILE_EXT", [], [[this.moduleModel.getOhPackageJson5Path()]]);
    }
    t && (t.endsWith(".d.ets") || t.endsWith(".d.ts") || this._log.warn(`The value of types must be a .d.ts or .d.ets file. at ${this.moduleModel.getName()} ${common_const_js_1.CommonConst.OH_PACKAGE_JSON5}.`));
  }
  checkWorkerConfig() {
    var e, t;
    const i = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.sourceOption) || undefined === t ? undefined : t.workers;
    if (!i) return;
    const o = build_option_path_info_js_1.buildOptionPath.getTargetBuildOptPath(this.moduleModel, this.targetName, "workers");
    i.forEach(e => {
      path_1.isAbsolute(e) && !file_util_js_1.FileUtil.fileExists(e) ? this._log.printErrorExit("WORKER_PATH_DOES_NOT_EXIST", [e, o], [[e], [e]]) : file_util_js_1.FileUtil.fileExists(file_util_js_1.FileUtil.convertToAbsolutePath(e, this.moduleModel.getProjectDir())) || this._log.printErrorExit("WORKER_PATH_DOES_NOT_EXIST", [e, o], [[e], [e]]), [".js", ".ts", ".ets"].includes(path_1.extname(e)) || this._log.printErrorExit("INVALID_WORKER_FILE_EXT", [e, o], [[e]]);
    });
  }
  versionCheck() {
    const e = validate_util_js_1.ValidateUtil.apiCompatibleCheck(this.projectModel, this.targetData, this.moduleModel),
      t = this.checkArkTSCompatibility(),
      i = e.suggestedVersion > t.suggestedVersion ? e.errorHandler : t.errorHandler;
    i && i();
  }
  ohPackageCheck() {
    const e = this.projectModel.isOhpmProject() ? oh_package_loader_js_1.ohPackageLoader.getNodeOhPackagePath(path_1.resolve(this.moduleModel.getProjectDir(), common_const_js_1.CommonConst.OH_PACKAGE_JSON5)) : this.moduleModel.getPackageJsonPath();
    if (fs_extra_1.existsSync(e)) {
      const t = oh_package_loader_js_1.ohPackageLoader.getOhPackageJsonObj(e),
        i = null == t ? undefined : t.version,
        o = /^[1-9]\d?(\.([1-9]?\d)){2}$/,
        s = typeof i;
      "string" !== s && this._log.printErrorExit("OH_PACKAGE_VERSION_NOT_STRING", [e, s]), i && !o.test(i) && this._log._buildError(`The version number of the '${null == t ? undefined : t.name}' module does not comply with the SemVer specification.\n               This will affect the installation and publication by ohpm.\n               Make sure the value of version in the oh-package.json5 file of the module complies with the SemVer specification.`)._printWarn(this.moduleName);
      const r = null == t ? undefined : t.name,
        n = /^(@(?![0-9-_])[a-z0-9-_]+(?<![-_])\/)?(?![0-9-_.])[a-z0-9-_.]+(?<![-_.])$/,
        a = typeof r;
      "string" !== a && this._log.printErrorExit("OH_PACKAGE_NAME_NOT_STRING", [e, a]), r && !n.test(r) && this._log._buildError(`The name of the '${null == t ? undefined : t.name}' module does not comply with the regular expression: ${n}.\n               This will affect the installation and publication by ohpm.\n               Make sure the value of name in the oh-package.json5 file of the module complies with the regular expression.`)._printWarn(this.moduleName);
    }
  }
  checkCodeTypeForWearable() {
    this.deviceTypes.includes(device_type_const_js_1.DeviceTypeConst.WEARABLE) && this.containsEts && this.compatibleApiVersion === sdk_const_js_1.ApiVersion.API_VERSION_8 && this._log._buildError("Wearable devices for API version 8 support only JS.")._printErrorAndExit(this.moduleModel.getName());
  }
  checkCodeTypeForLiteWearable() {
    (this.deviceTypes.includes(device_type_const_js_1.DeviceTypeConst.LITE_WEARABLE) || this.deviceTypes.includes(device_type_const_js_1.DeviceTypeConst.SMART_VISION)) && this.containsEts && this.compatibleApiVersion >= sdk_const_js_1.ApiVersion.API_VERSION_8 && ((0, common_util_js_1.isSingleFramework)() ? this._log.printErrorExit("LITE_WEARABLE_UNDER_API8_IN_SINGLE_FRAMEWORK_ONLY_SUPPORT_JS") : this._log._buildError("Lite-wearable devices with API version 8 or later support only JS.")._printErrorAndExit(this.moduleModel.getName()));
  }
  checkOhpmProjectSdkVersion() {
    if (this.isOhpmProject && !this.supportOHPM()) {
      const e = this.sdkInfo.getSdkVersion(),
        t = project_extra_info_service_js_1.ProjectExtraInfoService.getProjectRuntimeOS(this.projectModel) === runtime_type_enum_js_1.RuntimeTypeEnum.HARMONY_OS ? "SDK > HarmonyOS" : "SDK > OpenHarmony";
      this._log.printErrorExit("SDK_VERSION_TOO_LOW", [], [[e, t]]);
    }
  }
  checkApiVersionForUnitTest() {
    inject_util_js_1.InjectUtil.isUnitTestProcess() && this.apiType === hap_extra_info_js_1.ApiType.FA && this._log.printErrorExit("FA_MODE_DOES_NOT_SUPPORT_UNIT_TEST");
  }
  validateFastBuildAppOnFAMode() {
    var e, t, i;
    this.projectModel.isFaMode() && (null === (i = null === (t = null === (e = (0, find_target_product_js_1.findTargetProduct)(this.projectModel)) || undefined === e ? undefined : e.buildOption) || undefined === t ? undefined : t.packOptions) || undefined === i ? undefined : i.fastBuildApp) && this._log.printErrorExit("FA_MODE_DOES_NOT_SUPPORT_FAST_BUILD");
  }
  validateModuleName(e, t, i) {
    e !== t && this._log.printErrorExit("MULTIPLE_MODULE_NAME", [e, i, node_config_path_info_1.nodeConfigPathInfo.getNodeConfigPathByName(e)], [[e, t]]);
  }
  validateHarDeduplicate() {
    var e, t;
    if (!this.targetService.shouldEnableHarDeduplicate()) return;
    this.projectModel.isFaMode() && this._log.printErrorExit("HAR_DEDUPLICATE_FA_MODE_DOES_NOT_SUPPORT", [this.projectProfilePath]), this.compatibleApiVersion < 21 && this._log.printErrorExit("HAR_DEDUPLICATE_COMPATIBLEAPIVERSION_CHECK", [this.projectProfilePath]);
    (null === (e = this.targetService.getBuildOption().strictMode) || undefined === e ? undefined : e.useNormalizedOHMUrl) || this._log.printErrorExit("HAR_DEDUPLICATE_USENORMALIZED_OHMURL_MUST_SET", [this.projectProfilePath]);
    this.moduleModel.getJsonObjByTargetName(this.targetName).module.libIsolation && this._log.printErrorExit("HAR_DEDUPLICATE_LIBISOLATION_NOT_SUPPORTED", [this.moduleModel.getJsonProfilePath(this.targetName)]);
    let i = null === (t = this.targetService.getBuildOption().resOptions) || undefined === t ? undefined : t.idDefinedFilePath;
    if (i) {
      i = path_1.isAbsolute(i) ? i : path_1.resolve(this.projectModel.getProjectDir(), i), i.endsWith(".json5") && fs_extra_1.existsSync(i) || this._log.printErrorExit("HAR_DEDUPLICATE_ID_DEFINED_FILE_PATH_MUST_EXISTS", [i, this.projectProfilePath]);
      const e = this.sdkInfo.getResIdDefinedSchema();
      e && fs_extra_1.existsSync(e) && validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
        moduleName: this.projectModel.getName(),
        filePath: i,
        schemaPath: e
      });
    } else this._log.printErrorExit("HAR_DEDUPLICATE_ID_DEFINED_FILE_PATH_MUST_SET", [this.projectProfilePath]);
  }
  getBundleType() {
    var e, t, i;
    const o = (0, wdk_1.cloneDeep)(null === (e = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === e ? undefined : e.appOpt);
    return null !== (i = null !== (t = null == o ? undefined : o.bundleType) && undefined !== t ? t : this.targetData.getProduct().bundleType) && undefined !== i ? i : this.projectModel.getBundleType();
  }
  isAtomicServiceProject() {
    return this.getBundleType() === common_const_js_1.BundleType.ATOMIC_SERVICE;
  }
  checkArkTSCompatibility() {
    const e = this.moduleModel.getSourceSetByTargetName(this.targetData.getTargetName()).getCodeMap().get(code_type_enum_js_1.CodeType.ETS);
    return (null == e ? undefined : e.getSrcPath()) && this.compatibleApiVersion < 7 ? {
      suggestedVersion: 7,
      errorHandler: () => {
        this._log._buildError("The minimum compatible version of ArkTS is 7.")._solution("Modify build-profile.json5 compatibleSdkVersion to 7 or later.")._file(this.projectModel.getProfilePath())._printErrorAndExit();
      }
    } : {
      suggestedVersion: -1
    };
  }
  checkCustomTypesSupport() {
    const e = this.targetService.getCustomTypes();
    if (!e || Array.isArray(e) && 0 === e.length) return;
    this.compileApiVersion < sdk_const_js_1.ApiVersion.API_VERSION_10 && this._log._buildError("Only API version 10 or later supports custom types.")._detail(`Modify build-profile.json5 api version to ${sdk_const_js_1.ApiVersion.API_VERSION_10} or later.`)._file(this.projectModel.getProfilePath())._printErrorAndExit();
    this.moduleModel.getSourceSetByTargetName(this.targetData.getTargetName()).getCodeMap().get(code_type_enum_js_1.CodeType.ETS) || this._log.printErrorExit("TYPES_WITHOUT_ETS_CODE", [], [[], [sdk_const_js_1.ApiVersion.API_VERSION_10]]), this.checkCustomTypesIsValid(e);
  }
  checkCustomTypesIsValid(e) {
    this.checkCustomTypeExtName(e, this.dependencies), this.checkCustomTypePathIsValid(e, this.dependencies);
  }
  checkCustomTypePathIsValid(e, t) {
    const i = [];
    for (const o of e) t[o] || t[`@types/${o}`] || (0, build_profile_utils_js_1.getCustomTypePathIfExists)(this.moduleModel.getProjectDir(), o) || i.push(o);
    i.length > 0 && this._log.printErrorExit("TYPES_FILES_DO_NOT_EXIST", [i.join(","), build_option_path_info_js_1.buildOptionPath.getTargetBuildOptPath(this.moduleModel, this.targetName, "types")]);
  }
  checkCustomTypeExtName(e, t) {
    for (const i of e) {
      if (t[i] || t[`@types/${i}`]) continue;
      if ("" === path_1.extname(i)) continue;
      const e = path_1.isAbsolute(i) ? i : path_1.resolve(this.moduleModel.getProjectDir(), i);
      if (fs.existsSync(e)) {
        for (const e of common_const_js_1.CustomTypesConst.CUSTOM_TYPES_SUFFIX) i.endsWith(e) && this._log.printErrorExit("TYPES_FILES_WITH_REDUNDANT_EXT", [i, e, build_option_path_info_js_1.buildOptionPath.getTargetBuildOptPath(this.moduleModel, this.targetName, "types")], [[e, i]]);
        this._log.printErrorExit("INVALID_TYPES_FILES_EXT", [i, build_option_path_info_js_1.buildOptionPath.getTargetBuildOptPath(this.moduleModel, this.targetName, "types")]);
      }
    }
  }
  getModuleDynamicDependencies() {
    const e = this.isOhpmProject ? this.moduleModel.getOhPackageJson5Path() : this.moduleModel.getPackageJsonPath();
    let t = {};
    if (fs.existsSync(e)) {
      t = {
        dynamicDependencies: t,
        ...hvigor_1.Json5Reader.getJson5Obj(e).dynamicDependencies
      };
    }
    return t;
  }
  checkDynamicDependenciesSupport() {
    Object.keys(this.dynamicDependencies).length > 1 && (this.isFaMode || this.compileApiVersion < 9 || !this.containsEts) && this._log.printErrorExit("DYNAMIC_DEPENDENCIES_PROHIBIT", [this.moduleName]);
  }
  checkDimension(e, t) {
    var i;
    const o = [];
    null === (i = null == e ? undefined : e.forms) || undefined === i || i.forEach(e => {
      e.supportDimensions.includes(e.defaultDimension) || o.push(e.name);
    }), 0 !== o.length && this._log.printErrorExit("INVALID_DEFAULT_DIMENSION", [o.join('","'), t]);
  }
  checkEntryModules() {
    const e = this.service.getModuleModel(),
      t = e.getProfileOpt().entryModules,
      i = e.getProfilePath();
    this.apiType === hap_extra_info_js_1.ApiType.STAGE && t && this._log.warn(`In the stageMode project, the entryModules field configured in feature module is deprecated, but the compatibility of the old project is still maintained.\n         You can configured it in ${i} .`);
  }
  checkEmptyProperty() {
    [common_const_js_1.CommonConst.EMPTY_BUNDLENAME, common_const_js_1.HvigorConfigConst.OHOS_BYTE_CODE_HAR_OPTIMIZE, common_const_js_1.HvigorConfigConst.OHOS_DEFAULTS_AUTOLAZYIMPORT].forEach(e => {
      const t = hvigor_1.hvigorCore.getParameter().getPropertyInternal(e);
      t && "boolean" != typeof t && this._log.warn(`The configuration ${e} needs to be a boolean. Current type '${typeof t}' is invalid and has been defaulted to false.`);
    });
  }
  getFromConfigFilePathArr() {
    const e = [],
      t = this.jsonProfile.profile.module.extensionAbilities,
      i = null == t ? undefined : t.map(e => e.metadata);
    null == i || i.forEach(t => {
      const i = null == t ? undefined : t.map(e => e.resource);
      null == i || i.forEach(t => t ? e.push(t) : undefined);
    });
    const o = [];
    return e.forEach(e => {
      const t = this.pathInfo.getFromConfigPathByTargetName(this.moduleModel, this.targetName, (0, common_util_js_1.parsingProfileName)(e));
      t && fs_extra_1.existsSync(t) && o.push(t);
    }), o;
  }
  checkWidget() {
    this.isFaMode || this.targetData.getCompileApiVersion() < sdk_const_js_1.ApiVersion.API_VERSION_9 || this.targetName === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET || this.fromConfigFilePathArr.forEach(e => {
      const t = json_util_js_1.JsonUtil.getJson5Obj(e, "utf-8");
      this.checkDimension(t, e);
    });
  }
  checkRequestPermissions() {
    this.isFaMode || this.targetName === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET || this.checkLegalReqPermissions(this.jsonProfile.profile);
    const e = this.findDuplicatedPermissions(this.jsonProfile.profile);
    0 !== e.length && this._log.printErrorExit("DUPLICATED_PERMISSIONS", [this.isFaMode ? "reqPermissions" : "requestPermissions", e.join('","'), this.jsonProfile.jsonFilePath]);
  }
  checkRemoveRequestPermissions() {
    if (!this.isFaMode && this.targetName !== common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET) {
      const e = this.targetService.getBuildOption().removePermissions,
        t = this.jsonProfile.profile.module.definePermissions || [];
      if (undefined !== e && e.length > 0 && (this.moduleModel.isHapModule() || this.moduleModel.isHspModule())) {
        const i = this.getGrantData(t).allPermission;
        this.forEachCheckRemovePermissions(e, i);
      }
    }
  }
  forEachCheckRemovePermissions(e, t) {
    if (0 !== t.length) for (const i of e) t.includes(i.name) || this._log.warn(`The ${i.name} permission under removePermissions must be a value that is predefined within the SDK or a custom one that you have included under definePermissions.\nat ${build_option_path_info_js_1.buildOptionPath.getTargetBuildOptPath(this.moduleModel, this.targetName, "removePermissions")}`);else this._log.warn("No usable permissions predefined within the SDK or a custom one that you have included under definePermissions.");
  }
  findDuplicatedPermissions(e) {
    const t = [],
      i = [],
      o = this.isFaMode ? e.module.reqPermissions : e.module.requestPermissions;
    return null == o || o.forEach(e => {
      t.includes(e.name) && !i.includes(e.name) ? i.push(e.name) : t.push(e.name);
    }), i;
  }
  checkLegalReqPermissions(e) {
    const t = this.isFaMode ? e.module.reqPermissions : e.module.requestPermissions,
      i = e.module.definePermissions || [];
    undefined !== t && t.length > 0 && this.checkReqPermissionsByModuleType(t, i);
  }
  checkReqPermissionsByModuleType(e, t) {
    const {
      manualSettingsData: i,
      userGrantData: o,
      allPermission: s
    } = this.getGrantData(t);
    this.moduleModel.isHapModule() && (this.forEachCheckPermissions(e, s, o, common_const_js_1.GrantModeType.USER_GRANT, true), this.forEachCheckPermissions(e, s, i, common_const_js_1.GrantModeType.MANUAL_SETTINGS, true)), (this.moduleModel.isHspModule() || this.moduleModel.isHarModule()) && (this.forEachCheckPermissions(e, s, o, common_const_js_1.GrantModeType.USER_GRANT, false), this.forEachCheckPermissions(e, s, i, common_const_js_1.GrantModeType.MANUAL_SETTINGS, false));
  }
  getGrantData(e) {
    let t,
      i,
      o = [];
    if (this.isUseSdkPermission()) t = new Set(this.sdkInfo.getPreviewerUserGrant()), i = new Set(this.sdkInfo.getPreviewerManualSettings()), o = this.sdkInfo.getPreviewerPermissionNames();else {
      const e = json_reader_1.JsonReader.getJsonObj(path_1.resolve(__dirname, "../../../res/permissions/userGrantPermissions.json"), "utf-8").userGrantPermissions;
      t = new Set(e), i = new Set();
    }
    const s = new Set();
    return null == e || e.forEach(e => {
      e.name && !s.has(e.name) && (this.processPermissionByGrantMode(e, i, common_const_js_1.GrantModeType.MANUAL_SETTINGS), this.processPermissionByGrantMode(e, t, common_const_js_1.GrantModeType.USER_GRANT), o.push(e.name), s.add(e.name));
    }), {
      manualSettingsData: Array.from(i),
      userGrantData: Array.from(t),
      allPermission: o
    };
  }
  processPermissionByGrantMode(e, t, i) {
    e.name && (e.grantMode !== i && t.has(e.name) ? t.delete(e.name) : e.grantMode === i && t.add(e.name));
  }
  forEachCheckPermissions(e, t, i, o, s) {
    const r = this.isUseSdkPermission();
    for (const n of e) {
      if (r && !t.includes(n.name)) return void this._log.printErrorExit("THE_%S_PERMISSION_UNDER_REQUESTPERMISSIONS+MUST_BE_A_VALUE", [n.name, this.jsonProfile.jsonFilePath]);
      i.includes(n.name) && (!s || undefined !== n.reason && undefined !== n.usedScene || this._log.printErrorExit("THE_REASON_AND_USESCENE_ATTRIBUTES_ARE_MANDATORY_FOR_USER_GRANT_PERMISSIONS", [o, this.jsonProfile.jsonFilePath]), s || undefined !== n.reason || this._log.printErrorExit("THE_REASON_ATTRIBUTE_ARE_MANDATORY_FOR_USER_GRANT_PERMISSIONS", [o, this.jsonProfile.jsonFilePath]));
    }
  }
  isUseSdkPermission() {
    return this.sdkInfo.allowSdkPermission() && (this.compileApiVersion >= sdk_const_js_1.ApiVersion.API_VERSION_12 || this.targetData.isHarmonyOS());
  }
  isValidWidget(e, t, i) {
    return e ? file_util_js_1.FileUtil.fileExists(i) : file_util_js_1.FileUtil.fileExists(`${i}.hml`) || this.visualExists(`${i}.visual`);
  }
  visualExists(e) {
    const t = e.replace(`${path_1.sep}js${path_1.sep}`, `${path_1.sep}supervisual${path_1.sep}`);
    return file_util_js_1.FileUtil.fileExists(t);
  }
  checkExcludeSoFromInterfaceHar() {
    var e;
    (null === (e = this.targetService.getBuildOption().nativeLib) || undefined === e ? undefined : e.excludeSoFromInterfaceHar) && this.moduleModel.isHapModule() && this._log.warn(`excludeSoFromInterfaceHar only applies to HSPs. It is not effective for HAPs.\n              Check your nativeLib configuration in ${path_1.resolve(this.pathInfo.getModulePath(), "build-profile.json5")}.`);
  }
  checkCustomizedHar() {
    if (!this.targetService.isCustomizedHar()) return;
    this.targetService.isByteCodeHar() || this._log.printErrorExit("MANDATORY_BYTE_CODE_HAR_FOR_CUSTOMIZED_HAR", [this.moduleModel.getProfilePath()], []);
    const e = hvigor_1.coreParameter.properties[common_const_js_1.HvigorConfigConst.OHOS_COMPILE_LIB_ENTRY],
      t = path_1.resolve(hvigor_1.HVIGOR_PROJECT_WRAPPER_HOME, hvigor_1.DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME);
    e && this._log.printErrorExit("COMPILE_ENTRY_FILE_NOT_SUPPORTED", [t], []);
  }
}
