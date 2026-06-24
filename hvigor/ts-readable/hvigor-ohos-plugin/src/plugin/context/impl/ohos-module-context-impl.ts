/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/context/impl/ohos-module-context-impl.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import fs_extra_1 from "fs-extra";
import * as build_option_path_info_js_1 from "../../../common/build-option-path-info";
import * as core_project_profile_js_1 from "../../../common/global/core-project-profile";
import * as module_option_path_info_js_1 from "../../../common/module-option-path-info";
import * as ohos_trace_js_1 from "../../../common/trace/ohos-trace";
import * as common_const_js_1 from "../../../const/common-const";
import * as build_mode_manager_js_1 from "../../../project/build-option/build-mode-manager";
import * as inject_util_js_1 from "../../../utils/inject-util";
import * as har_module_build_profile_loader_js_1 from "../../../utils/loader/file/har-module-build-profile-loader";
import * as module_build_profile_loader_js_1 from "../../../utils/loader/file/module-build-profile-loader";
import * as oh_package_loader_js_1 from "../../../utils/loader/file/oh-package-loader";
import * as res_model_loader_js_1 from "../../../utils/loader/file/res-model-loader";
import * as ohos_logger_js_1 from "../../../utils/log/ohos-logger";
import * as custom_hook_service_js_1 from "../../hooks/custom-hook-service";
var __decorate = this && this.__decorate || function (e, t, o, r) {
  var l,
    i = arguments.length,
    n = i < 3 ? t : null === r ? r = Object.getOwnPropertyDescriptor(t, o) : r;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) n = Reflect.decorate(e, t, o, r);else for (var s = e.length - 1; s >= 0; s--) (l = e[s]) && (n = (i < 3 ? l(n) : i > 3 ? l(t, o, n) : l(t, o)) || n);
  return i > 3 && n && Object.defineProperty(t, o, n), n;
};
export class OhosModuleContextImpl {
  constructor(e) {
    var t;
    this.logger = ohos_logger_js_1.OhosLogger.getLogger(OhosModuleContextImpl.name), this.modulePlugin = e, this.taskService = this.modulePlugin.getTaskService(), this.targetArr = this.taskService.getTargets(), this.targetArtifactNames = this.taskService.getTargetArtifactNames(), this.moduleOhPackagePath = null === (t = this.modulePlugin.getModuleModel()) || undefined === t ? undefined : t.getOhPackageJson5Path();
  }
  targets(e) {
    this._targets(e);
  }
  targetsInternal(e) {
    this._targets(e);
  }
  _targets(e) {
    this.targetArr.forEach(t => e(t));
  }
  getModuleName() {
    return this._getModuleName();
  }
  getModuleNameInternal() {
    return this._getModuleName();
  }
  _getModuleName() {
    return this.modulePlugin.getModuleModel().getModule().getName();
  }
  getTargetArtifactNames() {
    return this._getTargetArtifactNames();
  }
  getTargetArtifactNamesInternal() {
    return this._getTargetArtifactNames();
  }
  _getTargetArtifactNames() {
    return this.targetArtifactNames;
  }
  getTargetArtifactName(e) {
    return this._getTargetArtifactName(e);
  }
  getTargetArtifactNameInternal(e) {
    return this._getTargetArtifactName(e);
  }
  _getTargetArtifactName(e) {
    return this.targetArtifactNames.get(e);
  }
  getModulePath() {
    return this._getModulePath();
  }
  getModulePathInternal() {
    return this._getModulePath();
  }
  _getModulePath() {
    return this.modulePlugin.getModuleModel().getModule().getNodeDir();
  }
  getModuleType() {
    return this._getModuleType();
  }
  getModuleTypeInternal() {
    return this._getModuleType();
  }
  _getModuleType() {
    return this.modulePlugin.getModuleModel().getModuleType();
  }
  getBuildProductRootPath() {
    return this._getBuildProductRootPath();
  }
  getBuildProductRootPathInternal() {
    return this._getBuildProductRootPath();
  }
  _getBuildProductRootPath() {
    return path_1.resolve(inject_util_js_1.InjectUtil.getModuleBuildRootPath(this.getModulePathInternal()), this.getModulePathInternal());
  }
  getOhpmDependencyInfo() {
    return this._getOhpmDependencyInfo();
  }
  getOhpmDependencyInfoInternal() {
    return this._getOhpmDependencyInfo();
  }
  _getOhpmDependencyInfo() {
    return this.taskService.getOhpmDependencyInfo();
  }
  getOhpmRemoteHspDependencyInfo(e = false) {
    return this._getOhpmRemoteHspDependencyInfo(e);
  }
  getOhpmRemoteHspDependencyInfoInternal(e = false) {
    return this._getOhpmRemoteHspDependencyInfo(e);
  }
  _getOhpmRemoteHspDependencyInfo(e = false) {
    const t = this.taskService.getProjectModel().getProject().getNodeDir(),
      o = core_project_profile_js_1.CoreProjectProfile.getInstance(t).getCurrentProductOpt();
    return this.taskService.getOhpmRemoteHspDependencies(this.taskService, o.name, e);
  }
  getBuildProfileOpt() {
    return this._getBuildProfileOpt();
  }
  getBuildProfileOptInternal() {
    return this._getBuildProfileOpt();
  }
  _getBuildProfileOpt() {
    return (0, wdk_1.cloneDeep)(this.taskService.getModuleModel().isHarModule() ? har_module_build_profile_loader_js_1.harModuleBuildProfileLoader.getBuildProfile(this.getModuleNameInternal()) : module_build_profile_loader_js_1.moduleBuildProfileLoader.getBuildProfile(this.getModuleNameInternal()));
  }
  getModuleJsonOpt() {
    return this._getModuleJsonOpt();
  }
  getModuleJsonOptInternal() {
    return this._getModuleJsonOpt();
  }
  _getModuleJsonOpt() {
    const e = path_1.resolve(this.taskService.getProjectModel().getProjectDir(), this.getModulePathInternal(), `src/main/${common_const_js_1.CommonConst.MODULE_JSON5}`);
    return (0, wdk_1.cloneDeep)(res_model_loader_js_1.resModelLoader.getModuleJson(e));
  }
  setBuildProfileOpt(e) {
    this._setBuildProfileOpt(e);
  }
  setBuildProfileOptInternal(e) {
    this._setBuildProfileOpt(e);
  }
  _setBuildProfileOpt(e) {
    var t;
    this.taskService.isFaMode() && this.logger.printErrorExit("FA_MODE_NOT_SUPPORT_MODIFY_FILE", ["buildProfile"]), this.taskService.getModuleModel().isHarModule() ? har_module_build_profile_loader_js_1.harModuleBuildProfileLoader.setBuildProfileJson5Obj(this.getModuleNameInternal(), e) : module_build_profile_loader_js_1.moduleBuildProfileLoader.setBuildProfileJson5Obj(this.getModuleNameInternal(), e), null === (t = this.modulePlugin.getModuleModel()) || undefined === t || t.refreshData(), this.modulePlugin.initBuildOptionMap((0, build_option_path_info_js_1.getHookFilePath)()), this.modulePlugin.refreshTargetServiceList();
  }
  setModuleJsonOpt(e) {
    this._setModuleJsonOpt(e);
  }
  setModuleJsonOptInternal(e) {
    this._setModuleJsonOpt(e);
  }
  _setModuleJsonOpt(e) {
    this.taskService.isFaMode() && this.logger.printErrorExit("FA_MODE_NOT_SUPPORT_MODIFY_FILE", ["module.json5"]);
    const t = path_1.resolve(this.taskService.getProjectModel().getProjectDir(), this.getModulePathInternal(), `src/main/${common_const_js_1.CommonConst.MODULE_JSON5}`);
    res_model_loader_js_1.resModelLoader.setModuleJson(t, e), module_option_path_info_js_1.moduleOptionPathInfo.setModuleOptPathMap(t, null == e ? undefined : e.module, (0, build_option_path_info_js_1.getHookFilePath)());
  }
  getDependenciesOpt() {
    return this._getDependenciesOpt();
  }
  getDependenciesOptInternal() {
    return this._getDependenciesOpt();
  }
  _getDependenciesOpt() {
    return (0, wdk_1.cloneDeep)(oh_package_loader_js_1.ohPackageLoader.getDependenciesOpt(this.moduleOhPackagePath));
  }
  getDevDependenciesOpt() {
    return this._getDevDependenciesOpt();
  }
  getDevDependenciesOptInternal() {
    return this._getDevDependenciesOpt();
  }
  _getDevDependenciesOpt() {
    return (0, wdk_1.cloneDeep)(oh_package_loader_js_1.ohPackageLoader.getDevDependenciesOpt(this.moduleOhPackagePath));
  }
  getDynamicDependenciesOpt() {
    return this._getDynamicDependenciesOpt();
  }
  getDynamicDependenciesOptInternal() {
    return this._getDynamicDependenciesOpt();
  }
  _getDynamicDependenciesOpt() {
    return (0, wdk_1.cloneDeep)(oh_package_loader_js_1.ohPackageLoader.getDynamicDependenciesOpt(this.moduleOhPackagePath));
  }
  setDependenciesOpt(e) {
    this._setDependenciesOpt(e);
  }
  setDependenciesOptInternal(e) {
    this._setDependenciesOpt(e);
  }
  _setDependenciesOpt(e) {
    oh_package_loader_js_1.ohPackageLoader.setDependenciesOpt(this.moduleOhPackagePath, e);
  }
  setDevDependenciesOpt(e) {
    this._setDevDependenciesOpt(e);
  }
  setDevDependenciesOptInternal(e) {
    this._setDevDependenciesOpt(e);
  }
  _setDevDependenciesOpt(e) {
    oh_package_loader_js_1.ohPackageLoader.setDevDependenciesOpt(this.moduleOhPackagePath, e);
  }
  setDynamicDependenciesOpt(e) {
    this._setDynamicDependenciesOpt(e);
  }
  setDynamicDependenciesOptInternal(e) {
    this._setDynamicDependenciesOpt(e);
  }
  _setDynamicDependenciesOpt(e) {
    oh_package_loader_js_1.ohPackageLoader.setDynamicDependenciesOpt(this.moduleOhPackagePath, e);
  }
  getVersion() {
    return this._getVersion();
  }
  getVersionInternal() {
    return this._getVersion();
  }
  _getVersion() {
    return oh_package_loader_js_1.ohPackageLoader.getVersion(this.moduleOhPackagePath);
  }
  setVersion(e) {
    this._setVersion(e);
  }
  setVersionInternal(e) {
    this._setVersion(e);
  }
  _setVersion(e) {
    oh_package_loader_js_1.ohPackageLoader.setVersion(this.moduleOhPackagePath, e);
  }
  loadCompilePlugin(e) {
    this._loadCompilePlugin(e);
  }
  loadCompilePluginInternal(e) {
    this._loadCompilePlugin(e);
  }
  _loadCompilePlugin(e) {
    var t, o;
    ohos_trace_js_1.ohosTrace.traceIsUseCompilePlugin(null === (t = this.modulePlugin.getModuleModel()) || undefined === t ? undefined : t.getName()), e || this.logger.printErrorExit("EMPTY_COMPILE_PLUGIN_PATH");
    const r = path_1.isAbsolute(e) ? e : path_1.resolve(this.modulePlugin.getModuleModel().getModule().getNodeDir(), e);
    fs_extra_1.existsSync(r) || this.logger.printErrorExit("PLUGIN_PATH_NOT_FOUND", [r]), r.endsWith(".ts") || this.logger.printErrorExit("PLUGIN_FILE_MUST_END_WITH_TS", [r]), (0, hvigor_1.doCompilePluginTypeCheck)(r), null === (o = this.modulePlugin.getModuleModel()) || undefined === o || o.setCompilePluginPath(r);
  }
  getBuildMode() {
    return this._getBuildMode();
  }
  getBuildModeInternal() {
    return this._getBuildMode();
  }
  _getBuildMode() {
    return build_mode_manager_js_1.BuildModeManager.getBuildMode();
  }
  transformAbc(e, t) {
    this._transformAbc(e, t);
  }
  transformAbcInternal(e, t) {
    this._transformAbc(e, t);
  }
  _transformAbc(e, t) {
    custom_hook_service_js_1.customHookActionService.registerCustomAction(this._getModulePath(), hvigor_arkts_compose_1.TRANSFORM_ABC_WORKER_REQUEST, e, t);
  }
}
__decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "targets", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "getModuleName", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "getTargetArtifactNames", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "getTargetArtifactName", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "getModulePath", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "getModuleType", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "getBuildProductRootPath", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "getOhpmDependencyInfo", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "getOhpmRemoteHspDependencyInfo", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "getBuildProfileOpt", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "getModuleJsonOpt", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "setBuildProfileOpt", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "setModuleJsonOpt", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "getDependenciesOpt", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "getDevDependenciesOpt", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "getDynamicDependenciesOpt", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "setDependenciesOpt", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "setDevDependenciesOpt", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "setDynamicDependenciesOpt", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "getVersion", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "setVersion", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "loadCompilePlugin", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "getBuildMode", null), __decorate([hvigor_1.TrackAPI], OhosModuleContextImpl.prototype, "transformAbc", null);
