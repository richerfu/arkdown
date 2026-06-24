/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/pre-build.js
 */
import fs_1 from "fs";
import os_1 from "os";
import * as path from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as fse from "fs-extra";
import * as build_option_path_info_js_1 from "../common/build-option-path-info";
import * as common_util_js_1 from "../common/common-util";
import * as module_ohos_config_manager_js_1 from "../common/global/module-ohos-config-manager";
import * as project_ohos_config_manager_js_1 from "../common/global/project-ohos-config-manager";
import * as module_dependency_util_js_1 from "../common/module-dependency-util";
import * as module_option_path_info_js_1 from "../common/module-option-path-info";
import * as ohos_trace_js_1 from "../common/trace/ohos-trace";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as inject_const_js_1 from "../const/inject-const";
import * as sdk_const_js_1 from "../const/sdk-const";
import * as module_type_enum_js_1 from "../enum/module-type-enum";
import * as hap_extra_info_js_1 from "../project/data/hap-extra-info";
import * as dependency_interface_js_1 from "../project/dependency/core/dependency-interface";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
import * as array_util_js_1 from "../utils/array-util";
import * as build_profile_utils_js_1 from "../utils/build-profile-utils";
import * as byte_code_har_utils_js_1 from "../utils/byte-code-har-utils";
import * as dependency_util_js_1 from "../utils/dependency-util";
import * as file_util_js_1 from "../utils/file-util";
import * as form_util_js_1 from "../utils/form-util";
import * as inject_util_js_1 from "../utils/inject-util";
import * as json_util_js_1 from "../utils/json-util";
import * as res_model_loader_js_1 from "../utils/loader/file/res-model-loader";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as mock_config_utils_js_1 from "../utils/mock-config-utils";
import * as ohos_test_util_js_1 from "../utils/ohos-test-util";
import * as task_util_js_1 from "../utils/task-util";
import * as error_handlers_js_1 from "../utils/validate/error-handlers";
import * as validate_config_js_1 from "../utils/validate/validate-config";
import * as validate_util_js_1 from "../utils/validate/validate-util";
import * as abstract_pre_build_js_1 from "./abstract/abstract-pre-build";
import * as task_names_js_1 from "./common/task-names";
import * as target_task_service_js_1 from "./service/target-task-service";
var __decorate = this && this.__decorate || function (e, t, i, o) {
  var s,
    r = arguments.length,
    a = r < 3 ? t : null === o ? o = Object.getOwnPropertyDescriptor(t, i) : o;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) a = Reflect.decorate(e, t, i, o);else for (var n = e.length - 1; n >= 0; n--) (s = e[n]) && (a = (r < 3 ? s(a) : r > 3 ? s(t, i, a) : s(t, i)) || a);
  return r > 3 && a && Object.defineProperty(t, i, a), a;
};
var Task = task_names_js_1.TaskNames.Task;
const _log = ohos_logger_js_1.OhosLogger.getLogger("hvigor-preBuild");
export class PreBuild extends abstract_pre_build_js_1.AbstractPreBuild {
  get isExpandImportEnabled() {
    var e, t;
    return !this.moduleModel.isHarModule() && !!(null === (t = null === (e = this.targetService.getBuildOption().arkOptions) || undefined === e ? undefined : e.expandImportPath) || undefined === t ? undefined : t.enable);
  }
  get excludeExpandImportDependencyNames() {
    var e, t;
    return this.isExpandImportEnabled && (null === (t = null === (e = this.targetService.getBuildOption().arkOptions) || undefined === e ? undefined : e.expandImportPath) || undefined === t ? undefined : t.exclude) || [];
  }
  declareInputs() {
    var e, t, i, o, s, r, a, n, l, d;
    let h = super.declareInputs();
    const _ = Object.keys(this.getDependencies(this.moduleModel, false)),
      c = Object.keys(this.getDependencies(this.projectModel, false)),
      u = (0, build_profile_utils_js_1.getRuntimeOnlyObjMap)(this.module.getNodeDir(), null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.runtimeOnly, (0, array_util_js_1.getUnionSet)(_, c));
    for (const [e, t] of u) h.set(e, t);
    const g = project_ohos_config_manager_js_1.projectOhosConfigManager.getConfig();
    g && h.set("appHvigorFileConfig", (0, hvigor_1.hash)(JSON.stringify(g)));
    const m = module_ohos_config_manager_js_1.moduleOhosConfigManager.getConfig(this.moduleName);
    return m && h.set("moduleHvigorFileConfig", (0, hvigor_1.hash)(JSON.stringify(m))), this.handleWidgetSrc(h), h.set(common_const_js_1.CommonConst.USE_NORMALIZED_OHM_URL, !!(null === (o = null === (i = this.targetService.getBuildOption()) || undefined === i ? undefined : i.strictMode) || undefined === o ? undefined : o.useNormalizedOHMUrl)), h.set(build_directory_const_js_1.BuildDirConst.INTEGRATED_HSP, !!(null === (s = this.targetService.getBuildOption().arkOptions) || undefined === s ? undefined : s.integratedHsp)), h.set("sourceRoots", this.extendSourceValidateResult.isValid), h.set(build_directory_const_js_1.BuildDirConst.TRANSFORM_LIB, null !== (n = null === (a = null === (r = this.targetService.getBuildOption()) || undefined === r ? undefined : r.arkOptions) || undefined === a ? undefined : a.transformLib) && undefined !== n ? n : ""), h.set("byteCodeHar", !!(null === (d = null === (l = this.targetService.getBuildOption()) || undefined === l ? undefined : l.arkOptions) || undefined === d ? undefined : d.byteCodeHar)), h = this.addConfigurationToInput(h), fse.existsSync(this.moduleModel.getMockConfigPath()) && h.set("mockConfigSources", (0, mock_config_utils_js_1.getMockConfigSources)(this.moduleModel.getMockConfigPath(), this.moduleModel.getProjectDir()).join(",")), h;
  }
  handleWidgetSrc(e) {
    this.formJsonPathArr.forEach(t => {
      const i = hvigor_1.ParseJsonFile.parseJsonFile(t, false);
      i.forms && i.forms.forEach(t => {
        if (!t.src) return;
        const i = "arkts" === t.uiSyntax ? path.resolve(this.moduleModel.getProjectDir(), "src/main", t.src) : path.resolve(this.moduleModel.getProjectDir(), "src/main", `${t.src}.json`);
        fs_1.existsSync(i) && e.set(t.src, i);
      });
    });
  }
  declareInputFiles() {
    const e = new hvigor_1.FileSet().addEntries([this.appJson5Path, this.targetJsonPath, ...this.formJsonPathArr, ...this.startWindowJsonPathArr], {
      isDirectory: false
    });
    this.setIncrementalInput(e, this.projectProfilePath), this.setIncrementalInput(e, this.profilePath), this.setIncrementalInput(e, this.appStartupPath), this.setIncrementalInput(e, this.routerMapJsonPath), this.setIncrementalInput(e, this.shareConfigJsonPath), this.targetService.getWorkerConfig().forEach(t => {
      this.setIncrementalInput(e, t);
    }), this.setIncrementalInput(e, this.syscapJsonPath), this.setIncrementalInput(e, this.pagesJsonPath), this.setIncrementalInput(e, this.systemThemeJsonPath), this.setIncrementalInput(e, this.insightIntentJsonPath), this.setIncrementalInput(e, this.arkDataPath, {
      isDirectory: true
    }), this.setIncrementalInput(e, this.utdPath, {
      isDirectory: true
    }), this.setIncrementalInput(e, this.hvigorConfigPath, {
      isDirectory: true
    }), this.setIncrementalInput(e, this.moduleModel.getMockConfigPath(), {
      isDirectory: false
    });
    const t = this.isOhpmProject ? this.moduleModel.getOhPackageJson5Path() : this.moduleModel.getPackageJsonPath();
    this.setIncrementalInput(e, t);
    const i = this.isOhpmProject ? this.projectModel.getOhPackageJson5Path() : this.projectModel.getPackageJsonPath();
    return this.setIncrementalInput(e, i), e;
  }
  setShortcutsJsonPathIncrementalInput(e) {
    this.shortcutsJsonPaths && this.shortcutsJsonPaths.forEach(t => {
      fse.existsSync(t) && e.addEntry(t);
    });
  }
  setIncrementalInput(e, t, i) {
    t && fs_1.existsSync(t) && e.addEntry(t, i);
  }
  constructor(e, t = Task.PRE_BUILD) {
    super(e, t), this.INSIGHT_INTENT_SRC_ENTRY = "insightIntent-srcEntry", this.MODULE_SRC_ENTRY = "module-srcEntry", this.MODULE_ABILITIES_SRC_ENTRY = "module-abilities-srcEntry", this.MODULE_EXTENSION_ABILITIES_SRC_ENTRY = "module-extensionAbilities-srcEntry", this.moduleDependencyNames = [];
    const i = this.projectModel.getAppRes();
    this.appJson5Path = i.getJsonPath();
    const o = e.getTargetData().getModuleSourceSetModel().getModuleTargetRes();
    this.targetJsonPath = o.getJsonPath(), this.targetModuleOptObj = o.getModuleJsonOpt(), this.formJsonPathArr = target_task_service_js_1.TargetTaskService.getFormJsonArr(e), this.resourcePath = o.getResourcePath(), this.routerMapJsonPath = e.getRouterMapJsonPath(this.moduleModel), this.shareConfigJsonPath = e.getShareConfigJsonPath(this.moduleModel), this.shortcutsJsonPaths = e.getShortcutsJsonPaths(this.moduleModel), this.insightIntentJsonPath = e.getInsightIntentJsonPath(), this.pagesJsonPath = this.getPagesJsonPath(), this.startWindowJsonPathArr = this.getStartWindowJsonPath(), this.systemThemeJsonPath = this.getSystemThemeJsonPath(), this.arkDataPath = path.resolve(e.getTargetData().getPathInfo().getModuleSrcMainResourceRawfileArkdataPath()), this.utdPath = path.resolve(e.getTargetData().getPathInfo().getModuleSrcMainResourceRawfileUtdPath()), this.hvigorConfigPath = path.resolve(hvigor_1.HVIGOR_PROJECT_WRAPPER_HOME, hvigor_1.DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME), this.isPreview = hvigor_1.hvigorCore.getExtraConfig().get(inject_const_js_1.InjectConst.BUILD_ROOT) === build_directory_const_js_1.BuildDirConst.PREVIEW_BUILD_PATH, this.extendSourceValidateResult = this.getExtendSourceDirsValidateResult(), this.mockCacheClean();
  }
  doValidateForDiffApiType() {
    var e;
    if (this.appJson5Validate(), this.moduleJson5Validate(), this.systemThemeJsonValidate(), this.startWindowJsonValidate(), this.formJsonValidate(), this.pagesJsonValidate(), this.routerMapJsonValidate(), this.shareConfigJsonValidate(), this.checkShareConfigUri(), this.hvigorFileConfigValidate(), this.sysCapValidate(), this.hvigorConfigValidate(), this.validateMockConfig(), this.checkAppConfiguration(), fs_1.existsSync(this.insightIntentJsonPath) && (this.buildProfileJsonValidate(), this.insightIntentJsonValidate()), this.targetESVerisonValidate(), fs_1.existsSync(this.utdPath)) {
      const e = fs_1.readdirSync(this.utdPath);
      if (undefined !== e && e.length > 0) for (const t of e) {
        const e = path.resolve(this.utdPath, t);
        this.utdJsonValidate(e);
      }
    }
    if (fs_1.existsSync(this.arkDataPath)) {
      const e = fs_1.readdirSync(this.arkDataPath);
      if (undefined !== e && e.length > 0) for (const t of e) {
        const e = path.resolve(this.arkDataPath, t);
        this.arkDataJsonValidate(e);
      }
    }
    (null === (e = this.service.getModuleModel()) || undefined === e ? undefined : e.isHapModule()) && this.validateMainElement(), this.checkOHMURL(), this.checkDynamicImportFile(), this.checkAllSrcEntry(), this.checkBundledDependencies(), this.checkUIAbility(), this.checkSdkVersionMatchLazyImport(), this.checkDynamicDependencyTypes(), this.checkIncludeAppScopeResInHapOrHar(), this.checkModulesOutsideProjectWithoutModuleInfo();
  }
  checkModulesOutsideProjectWithoutModuleInfo() {
    const e = this.projectModel.getAllModules().map(e => e.getName()),
      t = [];
    this.service.getAllDependencies().forEach(i => {
      this.isLocalModuleMissingInfo(i, e) && t.push(i.getDependencyName());
    }), t && 0 !== t.length && _log.warn(`Missing module info for local modules '${t}'. Check project-level build-profile.json5. `);
  }
  isLocalModuleMissingInfo(e, t) {
    return !e.getDependencyRootPath().includes(`${path.sep}oh_modules${path.sep}`) && !t.includes(e.getDependencyName()) && !e.isSODependency();
  }
  mockCacheClean() {
    const e = this.moduleModel.getMockConfigPath();
    if (!fse.existsSync(e) && this.isPreview) {
      const e = this.targetData.getTargetName(),
        t = this.targetData.getPathInfo(),
        i = t.getModulePreviewPath(),
        o = t.getModulePreviewIntermediatesResPath(),
        s = path.resolve(i, `cache/.${e}/mock-config.json`);
      fse.existsSync(s) && fse.unlinkSync(s);
      const r = path.resolve(i, `cache/.${e}/mock-config.json5`);
      fse.existsSync(r) && fse.unlinkSync(r);
      const a = path.resolve(o, "mock-config.json");
      fse.existsSync(a) && fse.unlinkSync(a);
    }
  }
  targetESVerisonValidate() {
    var e, t, i;
    (null === (i = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.tscConfig) || undefined === i ? undefined : i.targetESVersion) && (this.compileApiVersion < sdk_const_js_1.ApiVersion.API_VERSION_12 || this.compatibleApiVersion < sdk_const_js_1.ApiVersion.API_VERSION_12) && _log.printErrorExit("UNSUPPORTED_API_VERSION_TARGET_ES_VERSION", [this.projectProfilePath]);
  }
  checkByteCodeHar() {
    var e, t, i, o;
    if (!this.moduleModel.isHarModule()) return;
    const s = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.strictMode) || undefined === t ? undefined : t.useNormalizedOHMUrl;
    this.targetService.getByteCodeHar(s, null === (o = null === (i = this.targetService.getBuildOption()) || undefined === i ? undefined : i.arkOptions) || undefined === o ? undefined : o.byteCodeHar) && (ohos_trace_js_1.ohosTrace.traceByteCodeHar(this.moduleName), (0, byte_code_har_utils_js_1.checkByteCodeHar)({
      logger: _log,
      useNormalizedOHMUrl: s,
      compileApiVersion: this.compileApiVersion,
      compatibleApiVersion: this.compatibleApiVersion
    }));
  }
  buildProfileJsonValidate() {
    (this.compileApiVersion < sdk_const_js_1.ApiVersion.API_VERSION_11 || this.compatibleApiVersion < sdk_const_js_1.ApiVersion.API_VERSION_11) && _log.printErrorExit("UNSUPPORTED_API_VERSION_INSIGHT_INTENT", [this.targetJsonPath]);
  }
  insightIntentJsonValidate() {
    const e = this.sdkInfo.getInsightIntentSchema();
    fs_1.existsSync(e) || _log.printErrorExit("UNSUPPORTED_SDK_VERSION_INSIGHT_INTENT", [this.targetJsonPath]), validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.projectModel.getName(),
      filePath: this.insightIntentJsonPath,
      schemaPath: e
    });
  }
  utdJsonValidate(e) {
    const t = this.sdkInfo.getUtdSchema();
    fs_1.existsSync(t) || _log.printErrorExit("UNSUPPORTED_SDK_VERSION_ARK_DATA", [this.targetJsonPath]), validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.projectModel.getName(),
      filePath: e,
      schemaPath: t
    });
  }
  arkDataJsonValidate(e) {
    const t = this.sdkInfo.getArkDataSchema();
    fs_1.existsSync(t) || _log.printErrorExit("UNSUPPORTED_SDK_VERSION_ARK_DATA", [this.targetJsonPath]), validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.projectModel.getName(),
      filePath: e,
      schemaPath: t
    });
  }
  appJson5Validate() {
    const e = this.targetService.getSdkInfo().getAppSchema(),
      t = this.moduleModel.getModuleType() === module_type_enum_js_1.ModuleType.Har;
    validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.projectModel.getName(),
      filePath: this.appJson5Path,
      schemaPath: e,
      changeAppSchema: t
    });
  }
  validateModuleSrcEntry(e, t) {
    const i = t.module.srcEntry || t.module.srcEntrance;
    if (undefined === i) return;
    const o = this.moduleModel.getSourceSetByTargetName(this.targetData.getTargetName());
    undefined === t.module.srcEntry && _log.warn(`Field "module.srcEntrance" has been deprecated. Use "module.srcEntry" instead.\n      at ${o.getModuleTargetRes().getJsonPath()}`);
    const s = path.isAbsolute(i) ? i : path.resolve(o.getSourceSetRoot(), i);
    if (!file_util_js_1.FileUtil.fileExists(s) && !this.targetService.isCustomizedHar()) {
      const e = o;
      _log.printErrorExit("MODULE_SRC_ENTRY_NOT_FOUND", [s, e.getModuleTargetRes().getJsonPath()]);
    }
  }
  validateExecutableBinaryPaths(e) {
    const t = e.module.executableBinaryPaths;
    if (!t || 0 === t.length) return;
    const i = this.moduleModel.getProjectDir(),
      o = this.moduleModel.getSourceSetByTargetName(this.targetData.getTargetName()).getModuleTargetRes().getJsonPath(),
      s = [path.resolve(i, "libs/arm64-v8a"), path.resolve(i, "libs/x86_64"), path.resolve(i, "libs/armeabi-v7a")];
    for (const e of t) {
      path.isAbsolute(e.path) && _log.printErrorExit("EXECUTABLE_BINARY_PATH_SHOULD_BE_RELATIVE_PATH", [e, o]);
      const t = path.resolve(i, e.path);
      fse.existsSync(t) || _log.printErrorExit("EXECUTABLE_BINARY_PATH_FILE_IS_INVALID", [e, o], [[e]]), fse.statSync(t).isFile() || _log.printErrorExit("EXECUTABLE_BINARY_PATH_IS_NOT_FILE", [e, o]), this.isBinaryPathInAllowedDir(t, s) || _log.printErrorExit("EXECUTABLE_BINARY_PATH_NOT_FOUND_IN_LIBS", [e, o]);
    }
  }
  isBinaryPathInAllowedDir(e, t) {
    return t.some(t => {
      const i = path.relative(t, e);
      return !i.startsWith("..") && !path.isAbsolute(i);
    });
  }
  moduleJson5Validate() {
    const e = this.targetName,
      t = this.targetService.getSdkInfo().getModuleSchema();
    validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.moduleName,
      filePath: this.targetJsonPath,
      schemaPath: t,
      errorHandlers: [error_handlers_js_1.removeLegacyPhoneSupport, error_handlers_js_1.handleStartWindowErrors]
    }), e !== common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET && this.validateModuleName(this.moduleName, this.targetModuleOptObj.module.name, common_const_js_1.CommonConst.MODULE_JSON5);
    const i = hvigor_1.ParseJsonFile.parseJsonFile(this.targetJsonPath, true),
      o = hvigor_1.ParseJsonFile.parseJsonFile(this.targetJsonPath, false);
    this.validateModuleSrcEntry(_log, json_util_js_1.JsonUtil.getJson5Obj(this.targetJsonPath)), this.validateExecutableBinaryPaths(json_util_js_1.JsonUtil.getJson5Obj(this.targetJsonPath)), undefined === i.module.uiSyntax && this.sdkInfo.requireUISyntax() && _log.printErrorExit("UNSUPPORTED_SDK_VERSION_UI_SYNTAX", [this.targetJsonPath]), validate_util_js_1.ValidateUtil.validateHybridDeviceTypes(i.module.deviceTypes), validate_util_js_1.ValidateUtil.validateDuplicatedName(o.module.abilities, "ability", this.targetJsonPath);
  }
  checkDynamicImportFile() {
    var e, t;
    const i = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.runtimeOnly;
    if (!i) return;
    if ((this.compileApiVersion < sdk_const_js_1.ApiVersion.API_VERSION_11 || this.apiType === hap_extra_info_js_1.ApiType.FA) && _log.printErrorExit("UNSUPPORTED_API_VERSION_DYNAMIC_IMPORT", [this.moduleModel.getName(), common_const_js_1.CommonConst.PROFILE_JSON5, this.profilePath]), i.sources) {
      const e = this.targetService.isNoExternalImportByPath(),
        t = this.module.getNodeDir(),
        o = [],
        s = [],
        r = [],
        a = [],
        n = [],
        l = build_option_path_info_js_1.buildOptionPath.getTargetBuildOptPath(this.moduleModel, this.targetName, "sources");
      this.projectModel.getAllModules().forEach(e => {
        n.push(e.getModule().getNodeDir());
      }), i.sources.forEach(e => {
        const i = path.resolve(t, e);
        n.some(e => i.startsWith(e)) || a.push(e), fse.existsSync(i) ? fse.statSync(i).isFile() && !common_const_js_1.ValidateRegExp.DYNAMIC_IMPORT_FILE_SUFFIX_REG_EXP.test(file_util_js_1.FileUtil.getFileSuffix(e)) && r.push(e) : s.push(e), i.startsWith(this.module.getNodeDir()) || o.push(e);
      }), 0 !== s.length && _log.printErrorExit("SOURCES_NOT_FOUND", [this.moduleName, s, l]), 0 !== a.length && _log.printErrorExit("DYNAMIC_IMPORT_EXTERNAL_MODULE_PROHIBITED", [[...new Set(a)], this.moduleName, l]), 0 !== r.length && _log.printErrorExit("SOURCES_SUFFIX_INCORRECT", [r, this.moduleName, l]), 0 !== o.length && (e ? _log.printErrorExit("DYNAMIC_IMPORT_ACROSS_MODULE_PROHIBITED", [o, this.moduleName, l]) : _log.warn(`Invalid dynamic import configuration [${o}] in current module '${this.moduleName}'. \n   Detail: The sources configuration of runtimeOnly in module is not allowed to configure across modules.\n   at :${l}`));
    }
    const o = Object.keys({
        ...this.getDependencies(this.moduleModel, false),
        ...this.getDynamicDependencies(this.moduleModel)
      }),
      s = Object.keys({
        ...this.getDependencies(this.projectModel, false),
        ...this.getDynamicDependencies(this.projectModel)
      });
    i.packages && ((0, array_util_js_1.isSubset)((0, array_util_js_1.getUnionSet)(o, s), i.packages) || _log.printErrorExit("DYNAMIC_IMPORT_PATH_INVALID", [this.moduleModel.getName(), this.profilePath], [[this.moduleModel.getName(), common_const_js_1.CommonConst.OH_PACKAGE_JSON5]]));
  }
  checkAllSrcEntry() {
    var e, t;
    const i = new Map(),
      o = res_model_loader_js_1.resModelLoader.getModuleJson(this.targetJsonPath);
    o.module.srcEntry && i.set(this.MODULE_SRC_ENTRY, [o.module.srcEntry]);
    const s = o.module.abilities,
      r = [];
    if (null == s ? undefined : s.length) {
      for (const e of s) e.srcEntry && r.push(e.srcEntry);
      i.set(this.MODULE_ABILITIES_SRC_ENTRY, r);
    }
    const a = o.module.extensionAbilities,
      n = [];
    if (a) {
      for (const e of a) e.srcEntry && n.push(e.srcEntry);
      i.set(this.MODULE_EXTENSION_ABILITIES_SRC_ENTRY, n);
    }
    const l = [];
    if (fse.existsSync(this.insightIntentJsonPath)) {
      const o = json_util_js_1.JsonUtil.getJson5Obj(this.insightIntentJsonPath);
      for (const i of [...(null !== (e = o.insightIntents) && undefined !== e ? e : []), ...(null !== (t = o.insightIntentsSrcEntry) && undefined !== t ? t : [])]) i.srcEntry && l.push(i.srcEntry);
      i.set(this.INSIGHT_INTENT_SRC_ENTRY, l);
    }
    i.forEach((e, t) => {
      (null == e ? undefined : e.length) && this.validateSrcEntryList(e, t);
    });
  }
  validateSrcEntryList(e, t) {
    const i = t === this.INSIGHT_INTENT_SRC_ENTRY ? this.insightIntentJsonPath : this.targetJsonPath;
    null == e || e.forEach(e => {
      t === this.MODULE_EXTENSION_ABILITIES_SRC_ENTRY && e.endsWith(".so") ? this.validateExtensionAbilitiesSrcEntryForSo(t, e, i) : common_const_js_1.ValidateRegExp.RELATIVE_PATH_REG_EXP.test(e) || _log.printErrorExit("SOURCE_ENTRY_INVALID_SYNTAX", [t, i]);
    });
  }
  validateExtensionAbilitiesSrcEntryForSo(e, t, i) {
    this.moduleModel.isHarModule() && _log.printErrorExit("SOURCE_ENTRY_SO_NOT_SUPPORTED_IN_HAR", [e, i]);
    const o = path.isAbsolute(t),
      s = t.startsWith("/"),
      r = t.includes("./") || t.includes("../") || t.includes("\\");
    (o || s || r) && _log.printErrorExit("SOURCE_ENTRY_SO_INVALID_PATH", [e, i]);
  }
  formJsonValidate() {
    this.formJsonPathArr.forEach(e => {
      validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
        moduleName: this.moduleName,
        filePath: e,
        schemaPath: this.sdkInfo.getFormSchema()
      }), this.validateFormSrc(e);
    });
  }
  setModuleDependencyNames() {
    var e, t;
    if (!this.excludeExpandImportDependencyNames.length) return;
    const i = new Set();
    this.projectModel.getAllModules().forEach(e => {
      var t;
      if (!e.isHarModule()) return;
      const o = null === (t = (0, dependency_util_js_1.getJson5Obj)(e.getOhPackageJson5Path())) || undefined === t ? undefined : t.name;
      o && i.add(o);
    });
    if (!(null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.strictMode) || undefined === t ? undefined : t.useNormalizedOHMUrl)) {
      (inject_util_js_1.InjectUtil.isByteCodeHarOptimize() ? this.service.getAllModuleDependencies() : this.service.getModuleDependencies()).forEach(e => {
        e.isHarDependency() && i.add(e.getDependencyName());
      });
    }
    this.moduleDependencyNames = [...i];
  }
  async beforeAlwaysAction() {
    await super.beforeAlwaysAction(), this.checkEntryModuleJson(), this.checkIntegratedHspHsp(), this.checkGenerateSharedTgz(), this.checkTransformLib(), this.setModuleDependencyNames(), module_dependency_util_js_1.moduleDependencyUtil.init(this.service, this.moduleModel, this.projectModel, this.isFaMode);
  }
  checkEntryModuleJson() {
    const e = hvigor_1.ParseJsonFile.parseJsonFile(this.targetJsonPath, true);
    if (e.module.uiSyntax) {
      const t = e.module.uiSyntax._line,
        i = e.module.uiSyntax._column + 1;
      _log._file(this.targetJsonPath).warn(`uiSyntax has been deprecated.${os_1.EOL}         at ${this.targetJsonPath}:${t}:${i}`);
    }
  }
  getProfileJsonPath(e) {
    const t = (0, common_util_js_1.parsingProfileName)(e);
    if (!t) return;
    const i = this.targetService.getTargetResourceBaseProfilePath(`${t}.json`);
    return i && fse.existsSync(i) || _log.printErrorExit("PROFILE_NOT_FOUND", [`${t}.json`], [[], [this.targetName], [`${t}.json`]]), i;
  }
  getSystemThemeJsonPath() {
    var e;
    const t = null === (e = this.targetModuleOptObj) || undefined === e ? undefined : e.module.systemTheme;
    return this.getProfileJsonPath(null != t ? t : "");
  }
  getPagesJsonPath() {
    const e = hvigor_1.ParseJsonFile.parseJsonFile(this.targetJsonPath, false).module.pages;
    return this.getProfileJsonPath(e);
  }
  getStartWindowJsonPath() {
    var e;
    const t = hvigor_1.ParseJsonFile.parseJsonFile(this.targetJsonPath, false),
      i = null === (e = null == t ? undefined : t.module) || undefined === e ? undefined : e.abilities;
    if (!i) return [];
    const o = [];
    for (const e of i) if (e.startWindow) {
      const t = this.getProfileJsonPath(e.startWindow);
      t && o.push(t);
    }
    return o;
  }
  pagesJsonValidate() {
    const e = this.getPagesJsonPath();
    undefined !== e && validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.moduleName,
      filePath: e,
      schemaPath: this.sdkInfo.getPageSchema()
    });
  }
  startWindowJsonValidate() {
    this.startWindowJsonPathArr.forEach(e => {
      validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
        moduleName: this.moduleName,
        filePath: e,
        schemaPath: this.sdkInfo.getStartWindowSchema()
      });
    });
  }
  systemThemeJsonValidate() {
    this.systemThemeJsonPath && validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.moduleName,
      filePath: this.systemThemeJsonPath,
      schemaPath: this.sdkInfo.getSystemThemeSchema()
    });
  }
  routerMapJsonValidate() {
    this.checkResReferenceConfig(common_const_js_1.CommonConst.ROUTER_MAP), this.routerMapJsonPath && fse.existsSync(this.routerMapJsonPath) && validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.moduleName,
      filePath: this.routerMapJsonPath,
      schemaPath: this.sdkInfo.getRouterMapSchema()
    });
  }
  shareConfigJsonValidate() {
    this.checkResReferenceConfig(common_const_js_1.CommonConst.SHARE_CONFIG), this.shareConfigJsonPath && fse.existsSync(this.shareConfigJsonPath) && validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.moduleName,
      filePath: this.shareConfigJsonPath,
      schemaPath: this.sdkInfo.getShareConfigSchema()
    });
  }
  checkShareConfigUri() {
    const e = this.targetService.getTargetShareConfigObjList(this.moduleModel);
    if (!e || e.length <= 1) return;
    const t = new Set();
    for (let i = 0; i < e.length; i++) {
      const o = e[i].uri;
      t.has(o) ? _log.printErrorExit("SHARE_CONFIG_URI_UNIQUE", [o, "uri", this.moduleModel.getName()], [["uri", this.moduleModel.getName(), o]]) : t.add(o);
    }
  }
  sysCapValidate() {
    !this.targetData.isHarmonyOS() && fse.existsSync(this.syscapJsonPath) && validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.moduleName,
      filePath: this.syscapJsonPath,
      schemaPath: this.sdkInfo.getSysCapSchema()
    });
  }
  hvigorConfigValidate() {
    const e = common_const_js_1.BuildProfileSchemaFileConst.HVIGOR_CONFIG_SCHEMA_PATH;
    fs_1.existsSync(this.hvigorConfigPath) && fs_1.existsSync(e) && validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.projectModel.getName(),
      filePath: this.hvigorConfigPath,
      schemaPath: e
    });
  }
  validateFormSrc(e) {
    var t, i;
    const o = hvigor_1.ParseJsonFile.parseJsonFile(e, false);
    if (undefined !== o.forms) for (const s of o.forms) {
      s.src || _log.printErrorExit("FORM_SRC_FIELD_MISSING", [e]);
      let o = this.moduleModel.getProjectDir();
      const r = null !== (i = null === (t = this.targetModuleOptObj) || undefined === t ? undefined : t.module.formWidgetModule) && undefined !== i ? i : "",
        a = this.projectModel.getModuleModelByName(r);
      a && (o = a.getProjectDir());
      const n = path.resolve(o, "src/main"),
        l = file_util_js_1.FileUtil.convertToAbsolutePath(s.src, n);
      path.normalize(l).startsWith(path.normalize(o)) || _log.printErrorExit("FORM_SRC_INVALID", [s.src, o, e]);
      const d = "arkts" === s.uiSyntax;
      this.isValidWidget(d, n, l) || _log.printErrorExit("FORM_SRC_NOT_FOUND", [s.src, e]);
    }
  }
  validateMainElement() {
    if (undefined === this.targetModuleOptObj || this.isNewOhosTestTemplate()) return;
    const e = this.targetModuleOptObj.module.mainElement;
    this.targetModuleOptObj.module.installationFree && undefined === e && _log.printErrorExit("EMPTY_MAIN_ELEMENT", [this.targetJsonPath]);
  }
  isNewOhosTestTemplate() {
    return this.targetName === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET && (0, ohos_test_util_js_1.isTemplatesAfterSourceCodeMigration)(this.moduleModel.getSourceRootByTargetName(common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET));
  }
  validateAtomicService() {
    var e, t, i, o;
    if (!this.isAtomicServiceProject()) return void _log.debug("current product is not Atomic service.");
    const s = this.getBundleType(),
      r = null === (t = null === (e = this.targetModuleOptObj) || undefined === e ? undefined : e.module) || undefined === t ? undefined : t.installationFree,
      a = this.targetService.getBuildOption().externalNativeOptions;
    this.getAtomicServiceErrorPath(s, r);
    this.targetData.isHarmonyOS() || (this.validateSpecificField(s, r), _log.printErrorExit("ATOMIC_SERVICE_INVALID_RUNTIME_OS", [this.projectModel.getProfilePath()])), (0, task_util_js_1.limitMinApiVersion)(this.targetData, sdk_const_js_1.ApiVersion.API_VERSION_10) || _log.printErrorExit("ATOMIC_SERVICE_UNSUPPORTED_API", [this.projectProfilePath]), a && _log.printErrorExit("ATOMIC_SERVICE_UNSUPPORTED_NATIVE", [this.profilePath]);
    const n = this.service.getModuleModel().getModuleType();
    n !== module_type_enum_js_1.ModuleType.Har && n !== module_type_enum_js_1.ModuleType.Shared && (this.formJsonPathArr.forEach(e => {
      const t = hvigor_1.ParseJsonFile.parseJsonFile(e, false);
      if (t.forms) for (const i of t.forms) i.uiSyntax && "arkts" === i.uiSyntax || _log.printErrorExit("ATOMIC_SERVICE_JS_WIDGET", [e]);
    }), undefined !== (null === (o = null === (i = this.targetModuleOptObj) || undefined === i ? undefined : i.module) || undefined === o ? undefined : o.mainElement) || this.targetName === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET && (0, ohos_test_util_js_1.isTemplatesAfterSourceCodeMigration)(this.moduleModel.getSourceRootByTargetName(common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET)) || _log.printErrorExit("EMPTY_MAIN_ELEMENT", [this.targetJsonPath]));
  }
  getAtomicServiceErrorPath(e, t) {
    let i = "";
    return undefined !== e && "app" !== e || t && (i = this.targetJsonPath), e !== common_const_js_1.BundleType.ATOMIC_SERVICE && t || (i = this.appJson5Path), i;
  }
  validateSpecificField(e, t) {
    e === common_const_js_1.BundleType.ATOMIC_SERVICE && t && _log.printErrorExit("ATOMIC_SERVICE_INVALID_RUNTIME_OS", [this.projectModel.getProfilePath()]);
  }
  validateMockConfig() {
    const e = json_util_js_1.JsonUtil.getJson5Obj(this.moduleModel.getMockConfigPath());
    e && Object.values(e).forEach(e => {
      const t = e.source;
      if (t) {
        const e = path.resolve(this.moduleModel.getProjectDir(), t);
        fse.existsSync(e) || _log.printErrorExit("SOURCE_NOT_FOUND", [e, this.moduleModel.getMockConfigPath()]);
      } else _log.printErrorExit("SOURCE_FILED_EMPTY", [this.moduleModel.getMockConfigPath()]);
    });
  }
  getJsonProfileByModel() {
    const e = res_model_loader_js_1.resModelLoader.getModuleJson(this.targetJsonPath);
    return {
      jsonFilePath: this.targetJsonPath,
      profile: e,
      deviceTypes: e.module.deviceTypes,
      deviceConfig: common_const_js_1.CommonConst.DEVICE_TYPES,
      configurationProfile: common_const_js_1.CommonConst.MODULE_JSON5
    };
  }
  hvigorFileConfigValidate() {
    const e = project_ohos_config_manager_js_1.projectOhosConfigManager.getConfig();
    undefined !== e && (0, validate_config_js_1.validateOhosProjectConfig)(e, `${this.module.getProject().getBuildFilePath()}.ts`);
    const t = module_ohos_config_manager_js_1.moduleOhosConfigManager.getConfig(this.moduleName);
    undefined !== t && (this.moduleModel.isHarModule() ? (0, validate_config_js_1.validateOhosHarModuleConfig)(t, `${this.module.getBuildFilePath()}.ts`) : (0, validate_config_js_1.validateOhosModuleConfig)(t, `${this.module.getBuildFilePath()}.ts`));
  }
  checkOHMURL() {
    var e, t;
    const i = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.strictMode) || undefined === t ? undefined : t.useNormalizedOHMUrl;
    ohos_trace_js_1.ohosTrace.traceUseNormalizedOHMUrl(i), i && !(0, task_util_js_1.limitMinApiVersion)(this.targetData, sdk_const_js_1.ApiVersion.API_VERSION_11) && _log.printErrorExit("USENORMAILZEDOHMURL_CAN_ONLY_BE_USED_IN_API12", [this.projectModel.getProfilePath()]);
    const o = this.targetService.getModuleService().getAllDependencies(),
      s = [];
    for (const e of o) this.isEqualOHMUrl(e, i) || s.push(e.getPackageName());
    s.length > 0 && _log.printErrorExit("USENORMAILZEDOHMURL_DO_NOT_MATCH", [s.join(), String(i)], [[s.join()]]);
  }
  isEqualOHMUrl(e, t) {
    return e.getDependencyType() !== dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP || dependency_manager_js_1.DependencyManager.isLocalDependency(e, this.moduleModelNodePaths) || !!t == !!e.getUseNormalizedOHMUrl();
  }
  getExtendSourceDirsValidateResult() {
    var e;
    let t = "",
      i = "";
    const o = this.targetService.getTargetData(),
      s = null === (e = o.getTargetOpt().source) || undefined === e ? undefined : e.sourceRoots;
    if (!(null == s ? undefined : s.length)) return {
      isValid: true,
      cause: t,
      detail: i
    };
    const r = this.pathInfo.getModulePath(),
      a = this.pathInfo.getModuleSrcPath(),
      n = this.pathInfo.getModuleSrcMainPath().split(path.sep).length,
      l = `targets[${this.moduleModel.getTargetOptions().findIndex(e => e.name === o.getTargetName())}].source.sourceRoots`,
      d = new Set();
    for (const e of s) {
      if (path.isAbsolute(e)) {
        t = `Invalid value '${e}' in module '${this.moduleModel.getName()}'.`, i = `Make sure the values of '${l}' are relative paths.`;
        break;
      }
      let o = path.resolve(r, e);
      if (!fse.existsSync(o)) {
        t = `Path '${e}' not found in module '${this.moduleModel.getName()}'.`, i = `Make sure the values of '${l}' are valid paths.`;
        break;
      }
      let s = fse.lstatSync(o);
      if (s.isSymbolicLink()) try {
        o = fse.realpathSync(o), s = fse.lstatSync(o);
      } catch {
        t = `Path '${e}' not found in module '${this.moduleModel.getName()}'.`, i = `Make sure the values of '${l}' are valid paths.`;
        break;
      }
      if (!s.isDirectory()) {
        t = `Invalid value '${e}' in module '${this.moduleModel.getName()}'.`, i = `Make sure the values of '${l}' are valid directory paths.`;
        break;
      }
      if (!file_util_js_1.FileUtil.isSubDir(a, o) || o.split(path.sep).length !== n) {
        t = `Invalid value '${e}' in module '${this.moduleModel.getName()}'.`, i = `Make sure the values of '${l}' are subdirectories of '${a}'.`;
        break;
      }
      if (d.has(o)) {
        t = `Invalid value '${e}' in module '${this.moduleModel.getName()}'.`, i = `Make sure the values of '${l}' are unique.`;
        break;
      }
      d.add(o);
    }
    return {
      isValid: !(t && i),
      cause: t,
      detail: i
    };
  }
  checkExtendSourceDirs() {
    const {
      isValid: e,
      cause: t,
      detail: i
    } = this.extendSourceValidateResult;
    e || _log.printErrorExit("BUILD_PROFILE_CONFIGURATION_ERROR", [t, this.profilePath], [[i]]);
  }
  checkIntegratedHspHsp() {
    var e, t;
    const i = null === (e = this.targetService.getBuildOption().arkOptions) || undefined === e ? undefined : e.integratedHsp,
      o = null === (t = this.targetService.getBuildOption().strictMode) || undefined === t ? undefined : t.useNormalizedOHMUrl;
    i && !(0, task_util_js_1.limitMinApiVersion)(this.targetData, sdk_const_js_1.ApiVersion.API_VERSION_11) && _log.printErrorExit("INTEGRATED_HSP_UNSUPPORTED_API", [this.moduleModel.getParentProject().getProfilePath()]), i && !o && _log.printErrorExit("INTEGRATED_HSP_REQUIRED_NORMALIZED_OHMURL", [this.projectModel.getProfilePath()]), i && this.service.getModuleModel().isHapModule() && _log.warn(`The integratedHsp does not take effect in the HAP ${this.moduleModel.getName()}.`);
  }
  checkGenerateSharedTgz() {
    undefined !== this.targetService.getBuildOption().generateSharedTgz && this.service.getModuleModel().isHapModule() && _log.warn(`The generateSharedTgz does not take effect in the HAP ${this.moduleModel.getName()}.`);
  }
  checkTransformLib() {
    var e, t;
    const i = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.transformLib;
    if (i) {
      ohos_trace_js_1.ohosTrace.traceIsUseTransformLib(this.moduleName), !inject_util_js_1.InjectUtil.isInTest() && this.targetService.isSourceCodeHar() && _log.warn("Only the byte code har supports to configure the 'transformLib' field."), path.isAbsolute(i) && _log.printErrorExit("TRANSFORM_LIB_SHOULD_BE_RELATIVE_PATH", [path.resolve(this.pathInfo.getModulePath(), "build-profile.json5")]);
      const e = path.resolve(this.pathInfo.getModulePath(), i);
      fse.existsSync(e) ? (fse.statSync(e).isFile() || _log.printErrorExit("TRANSFORM_LIB_IS_NOT_FILE", [i, path.resolve(this.pathInfo.getModulePath(), "build-profile.json5")]), this.checkTransformLibFileType(e.substring(e.lastIndexOf(".") + 1))) : _log.printErrorExit("TRANSFORM_LIB_FILE_NOT_EXIST", [i, path.resolve(this.pathInfo.getModulePath(), "build-profile.json5")]);
    }
  }
  checkTransformLibFileType(e) {
    (0, hvigor_1.isWindows)() && "dll" !== e && _log.printErrorExit("TRANSFORM_LIB_REQUIRES_A_X_FILE", [".dll", "Windows", path.resolve(this.pathInfo.getModulePath(), "build-profile.json5")]), ((0, hvigor_1.isMac)() || (0, hvigor_1.isLinux)()) && "so" !== e && _log.printErrorExit("TRANSFORM_LIB_REQUIRES_A_X_FILE", [".so", "Mac or Linux", path.resolve(this.pathInfo.getModulePath(), "build-profile.json5")]);
  }
  checkAppConfiguration() {
    const e = res_model_loader_js_1.resModelLoader.getAppJson(this.appJson5Path).app.configuration;
    if (!e) return;
    const t = this.getConfigurationFilePath(e);
    if (!t) return;
    const i = build_option_path_info_js_1.buildOptionPath.getAppConfigPath(this.appJson5Path, "configuration");
    fse.existsSync(t) || _log.printErrorExit("CONFIG_FILE_NOT_FOUND", [i]);
    const o = this.sdkInfo.getAppConfigurationSchema();
    fse.existsSync(o) || _log.printErrorExit("SCHEMA_FILE_CANNOT_BE_FOUND", [o]), validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.projectModel.getName(),
      filePath: t,
      schemaPath: o
    });
  }
  getConfigurationFilePath(e) {
    const t = (0, common_util_js_1.parsingProfileName)(e);
    if (!t) return "";
    const i = this.projectModel.getAppRes().getResourcePath();
    return path.resolve(i, "base", "profile", `${t}.json`);
  }
  addConfigurationToInput(e) {
    var t, i;
    const o = null === (t = res_model_loader_js_1.resModelLoader.getAppJson(this.appJson5Path).app) || undefined === t ? undefined : t.configuration;
    if (e.set("configuration", null != o ? o : ""), o && fse.existsSync(this.getConfigurationFilePath(o))) {
      const t = hvigor_1.Json5Reader.getJson5Obj(this.getConfigurationFilePath(o));
      e.set("configurationFileJson", null !== (i = JSON.stringify(t)) && undefined !== i ? i : "");
    } else e.set("configurationFileJson", "");
    return e;
  }
  checkResReferenceConfig(e) {
    const t = res_model_loader_js_1.resModelLoader.getModuleJson(this.targetJsonPath).module[e];
    if (!t) return;
    const i = (0, common_util_js_1.parsingProfileName)(t);
    if (!i) return;
    const o = this.targetService.getTargetResourceBaseProfilePath(`${i}.json`);
    o && fse.existsSync(o) || _log.printErrorExit("FIELD_FILE_NOT_FOUND", [e, module_option_path_info_js_1.moduleOptionPathInfo.getModuleOptPath(this.targetJsonPath, e)]);
  }
  checkBundledDependencies() {
    var e, t, i, o;
    if (!this.targetService.isBundledDependencies()) return;
    const s = null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.strictMode) || undefined === t ? undefined : t.useNormalizedOHMUrl;
    this.targetService.getByteCodeHar(s, null === (o = null === (i = this.targetService.getBuildOption()) || undefined === i ? undefined : i.arkOptions) || undefined === o ? undefined : o.byteCodeHar) || _log.printErrorExit("BYTE_CODE_HAR_UNSUPPORTED", [this.projectModel.getProfilePath()]);
  }
  checkUIAbility() {
    var e, t;
    const i = this.targetJsonPath;
    if (!(null === (t = null === (e = res_model_loader_js_1.resModelLoader.getModuleJson(i).module) || undefined === e ? undefined : e.abilities) || undefined === t ? undefined : t.length) || !this.moduleModel.isHspModule() && !this.moduleModel.isHarModule()) return;
    this.targetData.getApiMeta().compatibleSdkVersion.version < sdk_const_js_1.ApiVersion.API_VERSION_14 && _log.warn("The UIAbility configuration for HAR or HSP is only supported on the versions which API is greater than or equal to 14. Otherwise it may occur some problems.");
  }
  checkSdkVersionMatchLazyImport() {
    var e, t;
    (null === (t = null === (e = this.targetService.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.autoLazyImport) && (this.compatibleApiVersion < sdk_const_js_1.ApiVersion.API_VERSION_12 || 12 === this.compatibleApiVersion && ("beta1" === this.compatibleSdkVersionStage || "beta2" === this.compatibleSdkVersionStage)) && _log.printErrorExit("AUTO_LAZY_IMPORT_CANNOT_BE_CONFIGURED_FOR_THE_CURRENT_API_VERSION");
  }
  iterateDependencies(e) {
    const t = this.service.getHarDependencies(),
      i = this.service.getOtherDependencies(),
      o = [];
    return e.forEach(e => {
      (t.find(t => t.getDependencyName() === e) || i.find(t => t.getDependencyName() === e)) && o.push(e);
    }), o;
  }
  checkDynamicDependencyTypes() {
    const e = Object.keys(this.getDynamicDependencies(this.moduleModel)),
      t = Object.keys(this.getDynamicDependencies(this.projectModel)),
      i = [...this.iterateDependencies(e), ...this.iterateDependencies(t)];
    0 !== i.length && _log.warn(`dynamicDependencies is intended for dynamically distributed HSP dependencies. It is not recommended to configure HAR packages or third-party dependencies. Using package '${i}'.`);
  }
  checkIncludeAppScopeResInHapOrHar() {
    var e;
    (this.moduleModel.isHapModule() || this.moduleModel.isHarModule()) && false === (null === (e = this.targetService.getBuildOption().resOptions) || undefined === e ? undefined : e.includeAppScopeRes) && _log.warn(`'IncludeAppScopeRes'  configured at 'build-profile.json5' only works for HSP module, and will not take effect in HAP/HAR module '${this.moduleModel.getName()}.`);
  }
  checkFormModule() {
    this.moduleModel.isHapModule() && this.checkMixForm(), this.moduleModel.isFormWidgetModule(this.targetName) ? this.checkFormWidgetModule() : this.moduleModel.isFormExtensionModule(this.targetName) && this.checkFormExtensionModule();
  }
  checkMixForm() {
    this.projectModel.getAllModules().some(e => e.isFormWidgetModule(this.targetName) || e.isFormExtensionModule(this.targetName)) && this.checkNoFormAbility();
  }
  checkFormExtensionModule() {
    var e;
    const t = null === (e = this.targetModuleOptObj) || undefined === e ? undefined : e.module.formWidgetModule;
    if (!t) return;
    const i = this.projectModel.getModuleModelByName(t),
      o = null == i ? undefined : i.getJsonPathByTargetName(this.targetName),
      s = null == i ? undefined : i.getJsonObjByTargetName(this.targetName);
    i ? (i.isFormWidgetModule(this.targetName) || _log.printErrorExit("FORM_WIDGET_MODULE_INVALID_TYPE", [t, i.getModuleType(), this.targetJsonPath]), (null == s ? undefined : s.module.formExtensionModule) !== this.moduleName && _log.printErrorExit("FORM_MODULE_INVALID_MAPPING", [t, this.moduleName, t, o, this.moduleName, this.targetJsonPath], [[t, this.moduleName], [t, this.moduleName], []]), this.compatibleApiVersion < sdk_const_js_1.ApiVersion.API_VERSION_20 && _log.printErrorExit("FORM_MODULE_INVALID_API_VERSION", [sdk_const_js_1.ApiVersion.API_VERSION_20, this.projectProfilePath])) : _log.printErrorExit("MODULE_NOT_FOUND", [t, this.targetJsonPath]);
  }
  checkFormWidgetModule() {
    var e, t, i;
    const o = null !== (t = null === (e = this.targetModuleOptObj) || undefined === e ? undefined : e.module.formExtensionModule) && undefined !== t ? t : "",
      s = this.projectModel.getModuleModelByName(o),
      r = null == s ? undefined : s.getJsonObjByTargetName(this.targetName),
      a = null == s ? undefined : s.getJsonPathByTargetName(this.targetName);
    if (!s) return void _log.printErrorExit("MODULE_NOT_FOUND", [o, this.targetJsonPath]);
    s.isFormExtensionModule(this.targetName) || _log.printErrorExit("FORM_EXTENSION_MODULE_INVALID_TYPE", [o, s.getModuleType(), this.targetJsonPath]), (null == r ? undefined : r.module.formWidgetModule) !== this.moduleName && _log.printErrorExit("FORM_MODULE_INVALID_MAPPING", [o, this.moduleName, o, a, this.moduleName, this.targetJsonPath], [[o, this.moduleName], [o, this.moduleName], []]), this.checkNoFormAbility();
    const n = Object.keys(this.getDynamicDependencies(this.moduleModel));
    for (const e of [...Object.keys(this.dependencies), ...n]) {
      const t = this.projectModel.getModuleModelByName(e);
      t && t.isHspModule() && _log.printErrorExit("FORM_WIDGET_MODULE_INVALID_DEPENDENCY", [e, this.moduleModel.getOhPackageJson5Path()]);
    }
    (null == r ? undefined : r.module.deliveryWithInstall) || _log.printErrorExit("FORM_MODULE_INVALID_DELIVERY_WITH_INSTALL", [a]), (null === (i = this.targetModuleOptObj) || undefined === i ? undefined : i.module.deliveryWithInstall) || _log.printErrorExit("FORM_MODULE_INVALID_DELIVERY_WITH_INSTALL", [this.targetJsonPath]), this.compatibleApiVersion < sdk_const_js_1.ApiVersion.API_VERSION_20 && _log.printErrorExit("FORM_MODULE_INVALID_API_VERSION", [sdk_const_js_1.ApiVersion.API_VERSION_20, this.projectProfilePath]);
  }
  checkNoFormAbility() {
    for (const e of this.projectModel.getAllModules()) {
      const t = form_util_js_1.FormUtil.getFormConfigPathInModuleRes(e, this.targetName);
      e.isHapModule() && t && _log.printErrorExit("NO_FORM_ABILITY_ALLOWED", [t, e.getJsonPathByTargetName(this.targetName)]);
    }
  }
  checkExpandImportConfigs() {
    if (!this.excludeExpandImportDependencyNames.length) return;
    const e = new Set(this.moduleDependencyNames),
      t = this.excludeExpandImportDependencyNames.filter(t => !e.has(t));
    t.length && _log.printErrorExit("INVALID_EXCLUDE_EXPAND_IMPORT_CONFIGS", [t.join(",")], [[build_option_path_info_js_1.buildOptionPath.getTargetBuildOptPath(this.moduleModel, this.targetName, "arkOptions.expandImportPath.exclude")]]);
  }
}
__decorate([(0, hvigor_1.Input)()], PreBuild.prototype, "moduleDependencyNames", undefined), __decorate([(0, hvigor_1.Input)()], PreBuild.prototype, "isExpandImportEnabled", null), __decorate([(0, hvigor_1.Input)()], PreBuild.prototype, "excludeExpandImportDependencyNames", null);
