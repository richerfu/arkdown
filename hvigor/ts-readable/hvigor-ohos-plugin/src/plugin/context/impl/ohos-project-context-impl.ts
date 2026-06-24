/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/context/impl/ohos-project-context-impl.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as build_option_path_info_js_1 from "../../../common/build-option-path-info";
import * as core_product_js_1 from "../../../common/global/core-product";
import * as core_project_profile_js_1 from "../../../common/global/core-project-profile";
import * as build_directory_const_js_1 from "../../../const/build-directory-const";
import * as common_const_js_1 from "../../../const/common-const";
import * as build_mode_manager_js_1 from "../../../project/build-option/build-mode-manager";
import * as oh_package_loader_js_1 from "../../../utils/loader/file/oh-package-loader";
import * as project_build_profile_loader_js_1 from "../../../utils/loader/file/project-build-profile-loader";
import * as res_model_loader_js_1 from "../../../utils/loader/file/res-model-loader";
import * as inject_util_js_1 from "../../../utils/inject-util";
import * as global_project_data_service_js_1 from "../../../tasks/service/global-project-data-service";
var __decorate = this && this.__decorate || function (e, t, o, r) {
  var n,
    i = arguments.length,
    s = i < 3 ? t : null === r ? r = Object.getOwnPropertyDescriptor(t, o) : r;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) s = Reflect.decorate(e, t, o, r);else for (var p = e.length - 1; p >= 0; p--) (n = e[p]) && (s = (i < 3 ? n(s) : i > 3 ? n(t, o, s) : n(t, o)) || s);
  return i > 3 && s && Object.defineProperty(t, o, s), s;
};
export class OhosProjectContextImpl {
  constructor(e, t) {
    this.logger = hvigor_1.HvigorLogger.getLogger(OhosProjectContextImpl.name), this.projectPlugin = e, this.projectPath = this.projectPlugin.getProjectModel().getProject().getNodeDir(), this.taskService = t, this.projectOhPackagePath = path_1.resolve(this.projectPath, common_const_js_1.CommonConst.OH_PACKAGE_JSON5), res_model_loader_js_1.resModelLoader.initSchemaPath(this.taskService.getSdkInfo().getAppSchema(), this.taskService.getSdkInfo().getModuleSchema()), this.initCurrentProduct(), this.initBuildPath();
  }
  initCurrentProduct() {
    const e = core_project_profile_js_1.CoreProjectProfile.getInstance(this.projectPath).getCurrentProductOpt();
    this.currentProduct = new core_product_js_1.CoreProduct(e, this.taskService.getProjectModel());
  }
  initBuildPath() {
    this.buildRootPath = this.taskService.getPathInfo().getProjectBuildPath(), this.buildProductOutputPath = path_1.resolve(this.buildRootPath, build_directory_const_js_1.BuildDirConst.BUILD_OUTPUTS, this.currentProduct.getProductNameInternal());
  }
  isDebugLineEnable() {
    return this._isDebugLineEnable();
  }
  isDebugLineEnableInternal() {
    return this._isDebugLineEnable();
  }
  _isDebugLineEnable() {
    return inject_util_js_1.InjectUtil.isDebugLineEnable();
  }
  getFastBuildApp() {
    return this._getFastBuildApp();
  }
  getFastBuildAppInternal() {
    return this._getFastBuildApp();
  }
  _getFastBuildApp() {
    var e, t;
    return null !== (t = null === (e = this.taskService.getBuildOption().packOptions) || undefined === e ? undefined : e.fastBuildApp) && undefined !== t && t;
  }
  getAppWithSignedPkg() {
    return this._getAppWithSignedPkg();
  }
  getAppWithSignedPkgInternal() {
    return this._getAppWithSignedPkg();
  }
  _getAppWithSignedPkg() {
    var e, t;
    return null !== (t = null === (e = this.taskService.getBuildOption().packOptions) || undefined === e ? undefined : e.appWithSignedPkg) && undefined !== t && t;
  }
  getAllModuleNameHash() {
    return this._getAllModuleNameHash();
  }
  getAllModuleNameHashInternal() {
    return this._getAllModuleNameHash();
  }
  _getAllModuleNameHash() {
    return global_project_data_service_js_1.GlobalProjectDataService.getInstance().getAllModuleNameHash();
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
    var t;
    return this.taskService.getOhpmRemoteHspDependencies(this.taskService, null === (t = this.currentProduct) || undefined === t ? undefined : t.getProductNameInternal(), e);
  }
  getProjectName() {
    return this._getProjectName();
  }
  getProjectNameInternal() {
    return this._getProjectName();
  }
  _getProjectName() {
    return this.projectPlugin._project.getName();
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
  getProjectPath() {
    return this._getProjectPath();
  }
  getProjectPathInternal() {
    return this._getProjectPath();
  }
  _getProjectPath() {
    return this.projectPlugin._project.getNodeDir();
  }
  getCurrentProduct() {
    return this._getCurrentProduct();
  }
  getCurrentProductInternal() {
    return this._getCurrentProduct();
  }
  _getCurrentProduct() {
    return this.currentProduct;
  }
  getBuildProductOutputPath() {
    return this._getBuildProductOutputPath();
  }
  getBuildProductOutputPathInternal() {
    return this._getBuildProductOutputPath();
  }
  _getBuildProductOutputPath() {
    return this.buildProductOutputPath;
  }
  getBuildRootPath() {
    return this._getBuildRootPath();
  }
  getBuildRootPathInternal() {
    return this._getBuildRootPath();
  }
  _getBuildRootPath() {
    return this.buildRootPath;
  }
  getAppJsonOpt() {
    return this._getAppJsonOpt();
  }
  getAppJsonOptInternal() {
    return this._getAppJsonOpt();
  }
  _getAppJsonOpt() {
    return (0, wdk_1.cloneDeep)(res_model_loader_js_1.resModelLoader.getAppJson(path_1.resolve(this.projectPath, common_const_js_1.CommonConst.APP_SCOPE, common_const_js_1.CommonConst.APP_CONFIG)));
  }
  getBuildProfileOpt() {
    return this._getBuildProfileOpt();
  }
  getBuildProfileOptInternal() {
    return this._getBuildProfileOpt();
  }
  _getBuildProfileOpt() {
    return (0, wdk_1.cloneDeep)(project_build_profile_loader_js_1.projectBuildProfileLoader.getBuildProfile());
  }
  setAppJsonOpt(e) {
    this._setAppJsonOpt(e);
  }
  setAppJsonOptInternal(e) {
    this._setAppJsonOpt(e);
  }
  _setAppJsonOpt(e) {
    this.taskService.isFaMode() && this.logger.printErrorExit("FA_MODE_NOT_SUPPORT_MODIFY_FILE", ["app.json5"]);
    const t = path_1.resolve(this.projectPath, common_const_js_1.CommonConst.APP_SCOPE, common_const_js_1.CommonConst.APP_CONFIG);
    res_model_loader_js_1.resModelLoader.setAppJson(t, e), build_option_path_info_js_1.buildOptionPath.setAppConfigPath(t, e, (0, build_option_path_info_js_1.getHookFilePath)());
  }
  setBuildProfileOpt(e) {
    this._setBuildProfileOpt(e);
  }
  setBuildProfileOptInternal(e) {
    this._setBuildProfileOpt(e);
  }
  _setBuildProfileOpt(e) {
    var t;
    this.taskService.isFaMode() && this.logger.printErrorExit("FA_MODE_NOT_SUPPORT_MODIFY_FILE", ["buildProfile"]), project_build_profile_loader_js_1.projectBuildProfileLoader.checkBuildProfile(project_build_profile_loader_js_1.projectBuildProfileLoader.getBuildProfile(), e), project_build_profile_loader_js_1.projectBuildProfileLoader.setBuildProfileJson5Obj(this.getProjectNameInternal(), e), null === (t = this.projectPlugin.getProjectModel()) || undefined === t || t.refreshData(), this.projectPlugin.initBuildOptionMap((0, build_option_path_info_js_1.getHookFilePath)()), this.taskService.refreshData();
  }
  getDependenciesOpt() {
    return this._getDependenciesOpt();
  }
  getDependenciesOptInternal() {
    return this._getDependenciesOpt();
  }
  _getDependenciesOpt() {
    return (0, wdk_1.cloneDeep)(oh_package_loader_js_1.ohPackageLoader.getDependenciesOpt(this.projectOhPackagePath));
  }
  getDevDependenciesOpt() {
    return this._getDevDependenciesOpt();
  }
  getDevDependenciesOptInternal() {
    return this._getDevDependenciesOpt();
  }
  _getDevDependenciesOpt() {
    return (0, wdk_1.cloneDeep)(oh_package_loader_js_1.ohPackageLoader.getDevDependenciesOpt(this.projectOhPackagePath));
  }
  getDynamicDependenciesOpt() {
    return this._getDynamicDependenciesOpt();
  }
  getDynamicDependenciesOptInternal() {
    return this._getDynamicDependenciesOpt();
  }
  _getDynamicDependenciesOpt() {
    return (0, wdk_1.cloneDeep)(oh_package_loader_js_1.ohPackageLoader.getDynamicDependenciesOpt(this.projectOhPackagePath));
  }
  setDependenciesOpt(e) {
    this._setDependenciesOpt(e);
  }
  setDependenciesOptInternal(e) {
    this._setDependenciesOpt(e);
  }
  _setDependenciesOpt(e) {
    oh_package_loader_js_1.ohPackageLoader.setDependenciesOpt(this.projectOhPackagePath, e);
  }
  setDevDependenciesOpt(e) {
    this._setDevDependenciesOpt(e);
  }
  setDevDependenciesOptInternal(e) {
    this._setDevDependenciesOpt(e);
  }
  _setDevDependenciesOpt(e) {
    oh_package_loader_js_1.ohPackageLoader.setDevDependenciesOpt(this.projectOhPackagePath, e);
  }
  setDynamicDependenciesOpt(e) {
    this._setDynamicDependenciesOpt(e);
  }
  setDynamicDependenciesOptInternal(e) {
    this._setDynamicDependenciesOpt(e);
  }
  _setDynamicDependenciesOpt(e) {
    oh_package_loader_js_1.ohPackageLoader.setDynamicDependenciesOpt(this.projectOhPackagePath, e);
  }
  getOverrides() {
    return this._getOverrides();
  }
  getOverridesInternal() {
    return this._getOverrides();
  }
  _getOverrides() {
    return (0, wdk_1.cloneDeep)(oh_package_loader_js_1.ohPackageLoader.getOverrides(this.projectOhPackagePath));
  }
  setOverrides(e) {
    this._setOverrides(e);
  }
  setOverridesInternal(e) {
    this._setOverrides(e);
  }
  _setOverrides(e) {
    oh_package_loader_js_1.ohPackageLoader.setOverrides(this.projectOhPackagePath, e);
  }
  getSdkDetails() {
    return this._getSdkDetails();
  }
  getSdkDetailsInternal() {
    return this._getSdkDetails();
  }
  _getSdkDetails() {
    const e = this.taskService.getSdkInfo();
    return {
      isOhos: () => e.isOhos,
      getSdkDir: () => e.sdkDir,
      getSdkVersion: e.getSdkVersion.bind(e),
      getEtsComponentVersion: e.getEtsComponentVersion.bind(e),
      getEtsComponentReleaseType: e.getEtsComponentReleaseType.bind(e)
    };
  }
}
__decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "isDebugLineEnable", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getFastBuildApp", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getAppWithSignedPkg", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getAllModuleNameHash", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getOhpmDependencyInfo", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getOhpmRemoteHspDependencyInfo", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getProjectName", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getBuildMode", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getProjectPath", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getCurrentProduct", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getBuildProductOutputPath", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getBuildRootPath", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getAppJsonOpt", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getBuildProfileOpt", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "setAppJsonOpt", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "setBuildProfileOpt", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getDependenciesOpt", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getDevDependenciesOpt", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getDynamicDependenciesOpt", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "setDependenciesOpt", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "setDevDependenciesOpt", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "setDynamicDependenciesOpt", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getOverrides", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "setOverrides", null), __decorate([hvigor_1.TrackAPI], OhosProjectContextImpl.prototype, "getSdkDetails", null);
