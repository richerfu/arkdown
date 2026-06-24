/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/service/target-task-service.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as fse from "fs-extra";
import * as common_util_js_1 from "../../common/common-util";
import * as ohos_trace_js_1 from "../../common/trace/ohos-trace";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as build_mode_const_js_1 from "../../const/build-mode-const";
import * as common_const_js_1 from "../../const/common-const";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as aot_compile_mode_enum_js_1 from "../../enum/aot-compile-mode-enum";
import * as build_mode_manager_js_1 from "../../project/build-option/build-mode-manager";
import * as sdk_loader_js_1 from "../../sdk/sdk-loader";
import * as byte_code_har_utils_js_1 from "../../utils/byte-code-har-utils";
import * as cmake_util_js_1 from "../../utils/cmake/cmake-util";
import * as file_util_js_1 from "../../utils/file-util";
import * as form_util_js_1 from "../../utils/form-util";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as json_util_js_1 from "../../utils/json-util";
import * as res_model_loader_js_1 from "../../utils/loader/file/res-model-loader";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_util_js_1 from "../../utils/task-util";
import * as validate_util_js_1 from "../../utils/validate/validate-util";
const _log = ohos_logger_js_1.OhosLogger.getLogger("Hvigor-target-service");
export class TargetTaskService {
  constructor(e, t) {
    var o, i;
    this.targetData = e, this.buildOption = build_mode_manager_js_1.buildOptionManager.getTargetBuildOption(e.getModuleModel().getName(), e.getTargetName());
    const r = null === (i = null === (o = this.buildOption) || undefined === o ? undefined : o.arkOptions) || undefined === i ? undefined : i.autoLazyImport;
    ohos_trace_js_1.ohosTrace.traceIsAutoLazyImport(e.getModuleModel().getName(), r), this.moduleService = t;
    const s = e.getProduct(),
      l = this.moduleService.getProjectModel().getProfilePath(),
      n = (0, task_util_js_1.isAtomicServiceProject)(s, t.getProjectModel());
    validate_util_js_1.ValidateUtil.integrationVersionCheck(e.isHarmonyOS(), s, n, l), this._sdkInfo = sdk_loader_js_1.SdkLoader.getSdkInfo(this.targetData.getApiMeta().compileSdkVersion, !this.targetData.isHarmonyOS());
    const a = this.getBuildOption().externalNativeOptions;
    this.shouldBuildCmake = cmake_util_js_1.CmakeUtil.nativeTaskCondition(this.targetData.getModuleModel(), this.targetData, a);
  }
  refreshBuildOption() {
    var e, t;
    this.buildOption = build_mode_manager_js_1.buildOptionManager.getTargetBuildOption(this.targetData.getModuleModel().getName(), this.targetData.getTargetName());
    const o = null === (t = null === (e = this.buildOption) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.autoLazyImport;
    ohos_trace_js_1.ohosTrace.traceIsAutoLazyImport(this.targetData.getModuleModel().getName(), o);
  }
  buildTaskName(e, t) {
    return {
      ...e,
      name: `${computeTargetTaskName(null != t ? t : this.getTargetData().getTargetName(), e.name)}`
    };
  }
  getTargetData() {
    return this.targetData;
  }
  getBuildOption() {
    return this.buildOption;
  }
  isByteCodeHar() {
    var e, t, o;
    if (!this.moduleService.getModuleModel().isHarModule() || (0, byte_code_har_utils_js_1.shouldTreatHarAsHap)(this.targetData.getTargetName())) return false;
    const i = null === (t = null === (e = this.getBuildOption()) || undefined === e ? undefined : e.strictMode) || undefined === t ? undefined : t.useNormalizedOHMUrl,
      r = null === (o = this.getBuildOption().arkOptions) || undefined === o ? undefined : o.byteCodeHar;
    return this.getByteCodeHar(i, r);
  }
  isCustomizedHar() {
    var e, t;
    return !!(null === (t = null === (e = this.getBuildOption().packingOptions) || undefined === e ? undefined : e.customizedOptions) || undefined === t ? undefined : t.basePackage);
  }
  getByteCodeHar(e, t) {
    return !(!e || undefined !== t) || !(!e && !t) && !!t;
  }
  isNoExternalImportByPath() {
    var e, t, o, i;
    const r = null === (t = null === (e = this.getBuildOption()) || undefined === e ? undefined : e.strictMode) || undefined === t ? undefined : t.useNormalizedOHMUrl,
      s = null === (i = null === (o = this.buildOption) || undefined === o ? undefined : o.strictMode) || undefined === i ? undefined : i.noExternalImportByPath;
    return !(!r || undefined !== s) || !!s;
  }
  isSourceCodeHar() {
    return this.moduleService.getModuleModel().isHarModule() && !this.isByteCodeHar();
  }
  isBundledDependencies() {
    var e, t;
    return !!(null === (t = null === (e = this.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.bundledDependencies);
  }
  getNativeLibOption() {
    var e;
    return {
      ...this.getBuildOption().nativeLib,
      filter: {
        ...this.getBuildOption().napiLibFilterOption,
        ...(null === (e = this.getBuildOption().nativeLib) || undefined === e ? undefined : e.filter)
      }
    };
  }
  getWorkerConfig() {
    var e, t, o;
    return file_util_js_1.FileUtil.convertToAbsolutePaths(null !== (o = null === (t = null === (e = this.buildOption) || undefined === e ? undefined : e.sourceOption) || undefined === t ? undefined : t.workers) && undefined !== o ? o : [], this.moduleService.getModuleModel().getProjectDir());
  }
  getBuildProfileFields() {
    var e, t;
    return null === (t = null === (e = this.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.buildProfileFields;
  }
  getResourceDirs() {
    var e;
    const t = null === (e = this.targetData.getTargetOpt().resource) || undefined === e ? undefined : e.directories;
    if (!(null == t ? undefined : t.length)) {
      const e = path_1.resolve(this.targetData.getModuleSourceSetModel().getSourceSetRoot(), build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES);
      return fs_1.existsSync(e) ? this.convertResourceAbsolutePath([e]) : [];
    }
    return this.convertResourceAbsolutePath(t);
  }
  convertResourceAbsolutePath(e) {
    return e.map(e => {
      const t = path_1.isAbsolute(e) ? e : path_1.resolve(this.moduleService.getModuleModel().getProjectDir(), e);
      return validate_util_js_1.ValidateUtil.validateTargetCustomizeResources(e, t), t;
    });
  }
  getAnBuildMode() {
    var e, t, o, i, r;
    const s = this.targetData.getCompileApiVersion(),
      l = null !== (o = null === (t = null === (e = this.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.aotCompileMode) && undefined !== o ? o : null === (i = this.getBuildOption()) || undefined === i ? undefined : i.aotCompileMode,
      n = null === (r = this.getBuildOption().arkOptions) || undefined === r ? undefined : r.hostPGO;
    switch (s) {
      case sdk_const_js_1.ApiVersion.API_VERSION_9:
        return null != l ? l : aot_compile_mode_enum_js_1.AotCompileModeEnum.AOT_NULL;
      case sdk_const_js_1.ApiVersion.API_VERSION_10:
      default:
        return !this._sdkInfo.isOhos || this._sdkInfo.checkAotFixedSdkVersion() || n ? n ? aot_compile_mode_enum_js_1.AotCompileModeEnum.AOT_PARTIAL : aot_compile_mode_enum_js_1.AotCompileModeEnum.AOT_TYPE : aot_compile_mode_enum_js_1.AotCompileModeEnum.AOT_NULL;
    }
  }
  getApPath() {
    var e, t, o, i, r;
    return null !== (r = null !== (o = null === (t = null === (e = this.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.apPath) && undefined !== o ? o : null === (i = this.getBuildOption()) || undefined === i ? undefined : i.apPath) && undefined !== r ? r : this.targetData.getCompileApiVersion() >= sdk_const_js_1.ApiVersion.API_VERSION_10 ? "./modules.ap" : undefined;
  }
  getApAbsolutePath() {
    const e = this.getApPath(),
      t = this.targetData.getModuleModel().getProjectDir();
    return e ? path_1.resolve(t, e) : t;
  }
  getCustomTypes() {
    var e, t;
    return null === (t = null === (e = this.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.types;
  }
  isDebug() {
    const e = this.buildOption.debuggable;
    return undefined === e || e;
  }
  isOpenSource() {
    var e, t, o, i;
    if (!this.isDebug()) {
      const r = null === (i = null === (o = null === (t = null === (e = this.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.obfuscation) || undefined === o ? undefined : o.ruleOptions) || undefined === i ? undefined : i.enable;
      if (undefined === r || !r) return true;
      if (r) return false;
    }
    return true;
  }
  getShouldBuildCmake() {
    return this.shouldBuildCmake;
  }
  getBuildMode() {
    return this.isDebug() ? build_mode_const_js_1.BuildMode.DEBUG : build_mode_const_js_1.BuildMode.RELEASE;
  }
  needPackBin(e) {
    return e.hasLiteDeviceInTarget() && this.isDebug() && !hvigor_1.hvigorCore.isCommandEntryTask(common_const_js_1.CommonConst.ASSEMBLE_APP);
  }
  getSdkInfo() {
    return this._sdkInfo;
  }
  getModuleService() {
    return this.moduleService;
  }
  static getFormJsonArr(e) {
    var t, o;
    const i = e.getTargetData().getModuleSourceSetModel().getModuleTargetRes(),
      r = e.getModuleService().getModuleModel(),
      s = e.getTargetData().getTargetName();
    if (r.isFormExtensionModule(s)) return [];
    if (r.isFormWidgetModule(s)) {
      const l = form_util_js_1.FormUtil.getTargetModuleOptObj(e, i),
        n = r.getSourceSetByTargetName(s).getTargetResPath(),
        a = null !== (o = null === (t = form_util_js_1.FormUtil.getFormExtensionModuleByTargetName(r, s)) || undefined === t ? undefined : t.getJsonPathByTargetName(s)) && undefined !== o ? o : "";
      return this.getFormJsonArrByTargetResource(l, n, a);
    }
    return this.getFormJsonArrByTargetResource(i.getModuleJsonOpt(), r.getSourceSetByTargetName(s).getTargetResPath(), i.getJsonPath());
  }
  static getFormJsonArrByTargetResource(e, t, o) {
    var i;
    const r = [];
    return (null == e ? undefined : e.module.extensionAbilities) ? (null === (i = e.module.extensionAbilities) || undefined === i || i.forEach(e => {
      var i;
      if ("form" !== e.type) return;
      if (!(null === (i = e.metadata) || undefined === i ? undefined : i.length)) return void _log.printErrorExit("THE_METADATE_FIELD_IN_FORMEXTENSIONABILITY_CANNOT_BE_LEFT_BLANK_OR_AS_AN_EMPTY_ARRAY", [o], [[o]]);
      const s = e.metadata.find(e => "ohos.extension.form" === e.name);
      (null == s ? undefined : s.resource) ? e.metadata.forEach(e => {
        if (!e.resource) return;
        "string" != typeof e.resource && _log.printErrorExit("METADATA_RESOURCE_NOT_STRING", [o]);
        const i = `${e.resource.replace(/\$profile:/, "")}.json`,
          s = path_1.resolve(t, build_directory_const_js_1.BuildDirConst.BASE, build_directory_const_js_1.BuildDirConst.PROFILE, i);
        this.formJsonFileCheck(s, e, o), r.push(s);
      }) : _log.printErrorExit("IN_FORMEXTENSIONABILITY_METADATA_MUST_CONTAIN_AN_OBJECT_WITH_ITS_NAME", [o], [[o]]);
    }), r) : r;
  }
  static formJsonFileCheck(e, t, o) {
    if (!fs_1.existsSync(e)) {
      const e = fs_1.readFileSync(o).toString().split("\n");
      let i = -1,
        r = -1;
      e.some((e, o) => -1 !== (r = e.indexOf(t.resource)) && (i = o, true)), _log.printErrorExit("CAN_NOT_RESOLVE_SYMBOL", [t.resource, o, i + 1, r + 1 + t.resource.length]);
    }
  }
  getAppStartupFileName(e) {
    var t;
    let o;
    if (e) {
      o = this.targetData.getModuleSourceSetModel().getLegacyModuleTargetRes().getJsonPath();
    } else {
      o = this.targetData.getModuleSourceSetModel().getModuleTargetRes().getJsonPath();
    }
    const i = res_model_loader_js_1.resModelLoader.getModuleJson(o),
      r = null === (t = null == i ? undefined : i.module) || undefined === t ? undefined : t.appStartup;
    return (0, common_util_js_1.parsingProfileName)(r);
  }
  getAppStartupPath(e) {
    const t = this.getAppStartupFileName(e);
    if (t) return this.getTargetResourceBaseProfilePath(`${t}.json`);
  }
  getShortcutsJsonPaths(e) {
    const t = this.getShortcutsJsonFileNames(e);
    if (!t) return [];
    const o = [];
    return t.forEach(e => {
      var t;
      o.push(null !== (t = this.getTargetResourceBaseProfilePath(`${e}.json`)) && undefined !== t ? t : `${e}.json`);
    }), o;
  }
  getShortcutsJsonFileNames(e) {
    var t, o;
    if (!e) return [];
    const i = path_1.resolve(e.getModule().getNodeDir(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, common_const_js_1.CommonConst.MODULE_JSON5),
      r = res_model_loader_js_1.resModelLoader.getModuleJson(i),
      s = [],
      l = [];
    return (null === (t = null == r ? undefined : r.module) || undefined === t ? undefined : t.abilities) && this.handleAbilityMetadata(r.module.abilities, s), (null === (o = null == r ? undefined : r.module) || undefined === o ? undefined : o.extensionAbilities) && this.handleAbilityMetadata(r.module.extensionAbilities, s), s.forEach(e => {
      (0, common_util_js_1.parsingProfileName)(e) && l.push((0, common_util_js_1.parsingProfileName)(e));
    }), l;
  }
  handleAbilityMetadata(e, t) {
    e.forEach(e => {
      (null == e ? undefined : e.metadata) && e.metadata.forEach(e => {
        (null == e ? undefined : e.name) === common_const_js_1.CommonConst.SHORTCUTS_FIELD_NAME && e.resource && t.push(e.resource);
      });
    });
  }
  getShareConfigJsonPath(e) {
    const t = this.getShareConfigJsonFileName(e);
    if (t) return this.getTargetResourceBaseProfilePath(`${t}.json`);
  }
  getShareConfigJsonFileName(e) {
    if (!e) return;
    const t = path_1.resolve(e.getModule().getNodeDir(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, common_const_js_1.CommonConst.MODULE_JSON5),
      o = res_model_loader_js_1.resModelLoader.getModuleJson(t).module.crossAppSharedConfig;
    return (0, common_util_js_1.parsingProfileName)(o);
  }
  getTargetShareConfigObjList(e) {
    const t = this.getShareConfigJsonPath(e);
    if (!t || !fse.existsSync(t)) return;
    const o = json_util_js_1.JsonUtil.getJson5Obj(t),
      i = null == o ? undefined : o.crossAppSharedConfig;
    return Array.isArray(i) && i.forEach(t => {
      Object.assign(t, {
        moduleNodeDir: e.getModule().getNodeDir()
      });
    }), null == o ? undefined : o.crossAppSharedConfig;
  }
  getRouterMapJsonPath(e) {
    const t = this.getRouterMapJsonFileName(e);
    if (t) return this.getTargetResourceBaseProfilePath(`${t}.json`);
  }
  getInsightIntentJsonPath() {
    var e;
    return null !== (e = this.getTargetResourceBaseProfilePath(build_directory_const_js_1.BuildArtifactConst.INSIGHT_INTENT_JSON)) && undefined !== e ? e : path_1.resolve(this.getTargetData().getPathInfo().getModuleSrcMainResourceBaseProfilePath(), build_directory_const_js_1.BuildArtifactConst.INSIGHT_INTENT_JSON);
  }
  getRouterMapJsonFileName(e) {
    if (!e) return;
    const t = path_1.resolve(e.getModule().getNodeDir(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN, common_const_js_1.CommonConst.MODULE_JSON5),
      o = res_model_loader_js_1.resModelLoader.getModuleJson(t).module.routerMap;
    return (0, common_util_js_1.parsingProfileName)(o);
  }
  getTargetRouterMapObjList(e) {
    const t = this.getRouterMapJsonPath(e);
    if (!t || !fse.existsSync(t)) return;
    const o = json_util_js_1.JsonUtil.getJson5Obj(t);
    return o.routerMap.forEach(t => {
      Object.assign(t, {
        moduleNodeDir: e.getModule().getNodeDir()
      });
    }), null == o ? undefined : o.routerMap;
  }
  getTargetResourceBaseProfilePath(e) {
    for (const t of this.getResourceDirs()) {
      const o = path_1.resolve(t, common_const_js_1.CommonConst.BASE_PROFILE, e);
      if (fse.existsSync(o)) return o;
    }
  }
  getPkgContextInfo(e) {
    const t = this.targetData.getPathInfo().getIntermediatesPkgContextInfoPath(),
      o = json_util_js_1.JsonUtil.getJson5Obj(t);
    return o ? o[e] : {};
  }
  getAbsoluteSrcEntryPath(e, t) {
    return path_1.resolve(t, `${build_directory_const_js_1.BuildDirConst.SRC}${path_1.sep}${build_directory_const_js_1.BuildDirConst.MAIN}`, e);
  }
  shouldEnableHarDeduplicate() {
    var e;
    return !!(null === (e = this.getBuildOption().packOptions) || undefined === e ? undefined : e.deduplicateHar) && !inject_util_js_1.InjectUtil.isPreviewProcess() && !inject_util_js_1.InjectUtil.isInTest();
  }
  shouldDoDeduplicateTaskInDebuggingHsp() {
    if (!this.moduleService.getModuleModel().isHspModule()) return true;
    const e = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.WITH_DEDUPLICATE);
    return !e || "true" === e;
  }
  shouldDoDeduplicateTask() {
    return this.shouldEnableHarDeduplicate() && this.shouldDoDeduplicateTaskInDebuggingHsp();
  }
  isOhosTest() {
    return this.getTargetData().getTargetName() === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET;
  }
  isIntegratedHsp() {
    var e, t;
    return this.moduleService.getModuleModel().isHspModule() && !this.isOhosTest() && !!(null === (t = null === (e = this.getBuildOption()) || undefined === e ? undefined : e.arkOptions) || undefined === t ? undefined : t.integratedHsp);
  }
}
export function computeTargetTaskName(e, t) {
  return `${e}@${t}`;
}
