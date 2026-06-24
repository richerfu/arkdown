/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/project/core-project-model-impl.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as fse from "fs-extra";
import path_1 from "path";
import * as find_target_product_js_1 from "../../common/find-target-product";
import * as ohos_trace_js_1 from "../../common/trace/ohos-trace";
import * as common_const_js_1 from "../../const/common-const";
import * as version_const_js_1 from "../../const/version-const";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as build_mode_manager_js_1 from "../../project/build-option/build-mode-manager";
import * as sdk_util_js_1 from "../../sdk/sdk-util";
import * as array_util_js_1 from "../../utils/array-util";
import * as command_line_util_js_1 from "./util/command-line-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as validate_util_js_1 from "../../utils/validate/validate-util";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as oh_package_loader_js_1 from "../../utils/loader/file/oh-package-loader";
import * as project_build_profile_loader_js_1 from "../../utils/loader/file/project-build-profile-loader";
import * as dependency_param_validate_js_1 from "../../validate/dependency-param-validate";
import * as arkui_x_project_model_js_1 from "./arkui-x/arkui-x-project-model";
import * as common_util_1 from "../../common/common-util";
export class CoreProjectModelImpl {
  constructor(e) {
    this.project = e, this._log = ohos_logger_js_1.OhosLogger.getLogger(CoreProjectModelImpl.name), this.productApiMetaMap = new Map(), this._moduleSpecificTargets = new Map(), this.subModels = new Map(), this.targetRuntimeOS = [], this._moduleTargets = new Map(), this.projectPath = e.getNodeDir(), this.name = e.getName(), this._profilePath = path_1.resolve(this.projectPath, common_const_js_1.CommonConst.PROFILE_JSON5), this.projectBuildProfileCheck(this._profilePath), this._profileOptions = project_build_profile_loader_js_1.projectBuildProfileLoader.getBuildProfile(), this._productNames = this._profileOptions.app.products.map(e => e.name), this.projectStatusCheck(), this.initializeApiMetadata(), this.parseConfiguredTargetsInHvigorCommand(), this.arkuiProjectModel = new arkui_x_project_model_js_1.ArkUIXProjectModel(this.name, this.projectPath, this._profileOptions);
    let t = this.isOhpmProject();
    this.projectPackageJsonFilePath = t ? this.getOhPackageJson5Path() : this.getPackageJsonPath(), this.packageJsonObj = hvigor_1.Json5Reader.getJson5Obj(this.projectPackageJsonFilePath), this.trace(), t && this.initOhpmParameterFile();
  }
  initOhpmParameterFile() {
    const e = hvigor_1.hvigorCore.getParameter().getExtParamInternal(common_const_js_1.CommonConst.PARAMETER_FILE);
    e && !this.packageJsonObj.parameterFile && this._log.printErrorExit("NO_PARAMETER_FILE_CONFIG", [this.projectPackageJsonFilePath], [[this.projectPackageJsonFilePath]]);
    const t = (0, common_util_1.replaceStringParam)(null != e ? e : this.packageJsonObj.parameterFile, this.packageJsonObj.properties);
    t && (this.parameterFileAbsolutePath = path_1.resolve(this.getProjectDir(), t), fse.existsSync(this.parameterFileAbsolutePath) || this._log.printErrorExit("PARAMETER_FILE_NOT_EXIST", [t, this.projectPackageJsonFilePath]), fse.statSync(this.parameterFileAbsolutePath).isFile() || this._log.printErrorExit("PARAMETER_FILE_NOT_FILE", [t, this.projectPackageJsonFilePath]), this.parameterFileObj = hvigor_1.Json5Reader.getJson5Obj(this.parameterFileAbsolutePath), this.parameterFileObj && (0, dependency_param_validate_js_1.validateParameterKeys)(this.parameterFileObj));
  }
  trace() {
    ohos_trace_js_1.ohosTrace.traceBuildMode(build_mode_manager_js_1.BuildModeManager.getBuildMode()), ohos_trace_js_1.ohosTrace.traceProjectPackageJson(this.packageJsonObj);
  }
  refreshData() {
    this._profileOptions = project_build_profile_loader_js_1.projectBuildProfileLoader.getBuildProfile(), this._productNames = this._profileOptions.app.products.map(e => e.name), this.projectStatusCheck(), this.initializeApiMetadata();
  }
  registryTarget(e) {
    var t;
    const i = e.getModuleService().getModuleModel().getName(),
      r = null !== (t = this._moduleTargets.get(i)) && undefined !== t ? t : [];
    this._moduleTargets.set(i, [...r, e]);
  }
  getTarget(e, t) {
    var i, r;
    return t ? null === (r = this._moduleTargets.get(e)) || undefined === r ? undefined : r.find(e => e.getTargetData().getTargetName() === t) : null === (i = this._moduleTargets.get(e)) || undefined === i ? undefined : i.find(() => true);
  }
  getTargetRuntimeOSs() {
    return this.targetRuntimeOS;
  }
  parseConfiguredTargetsInHvigorCommand() {
    const e = hvigor_1.hvigorCore.getExtraConfig().get("module");
    this._moduleSpecificTargets = (0, command_line_util_js_1.parseTargets)(e);
  }
  projectStatusCheck() {
    validate_util_js_1.ValidateUtil.validateContainsDefault(this._profileOptions.app.products, this._profilePath), validate_util_js_1.ValidateUtil.validateDuplicatedName(this._profileOptions.app.products, "products", this._profilePath), validate_util_js_1.ValidateUtil.validateDuplicatedName(this._profileOptions.app.signingConfigs, "signingConfigs", this._profilePath), validate_util_js_1.ValidateUtil.validateDuplicatedName(this._profileOptions.modules, "modules", this._profilePath), this._profileOptions.modules.forEach(e => validate_util_js_1.ValidateUtil.validateDuplicatedName(e.targets, "targets", this._profilePath)), validate_util_js_1.ValidateUtil.apiConfigurationCheck(this), validate_util_js_1.ValidateUtil.modelVersionCheck();
  }
  initializeApiMetadata() {
    var e;
    const t = (0, find_target_product_js_1.findTargetProduct)(this),
      i = t.runtimeOS === common_const_js_1.CommonConst.HARMONY_OS;
    this.productApiMetaMap.set(t.name, {
      compileSdkVersion: (0, sdk_util_js_1.parseApiVersion)(null !== (e = t.compileSdkVersion) && undefined !== e ? e : version_const_js_1.VersionConst.SUPPORT_COMPILE_VERSION, i),
      compatibleSdkVersion: (0, sdk_util_js_1.parseApiVersion)(t.compatibleSdkVersion, i),
      targetSdkVersion: t.targetSdkVersion ? (0, sdk_util_js_1.parseApiVersion)(t.targetSdkVersion, i) : undefined,
      originCompatibleSdkVersion: t.compatibleSdkVersion
    });
  }
  getBuildProfileName() {
    return common_const_js_1.CommonConst.PROFILE_JSON5;
  }
  getName() {
    return this.name;
  }
  getProject() {
    return this.project;
  }
  getPackageJsonPath() {
    return path_1.resolve(this.projectPath, common_const_js_1.CommonConst.PACKAGE_JSON);
  }
  getOhPackageJson5Path() {
    return oh_package_loader_js_1.ohPackageLoader.getNodeOhPackagePath(path_1.resolve(this.projectPath, common_const_js_1.CommonConst.OH_PACKAGE_JSON5));
  }
  getProfilePath() {
    return this._profilePath;
  }
  getProjectDir() {
    return this.projectPath;
  }
  getProductNames() {
    return this._productNames;
  }
  getProfileOpt() {
    return this._profileOptions;
  }
  getSubModuleModels() {
    return this.subModels;
  }
  getModuleModelByName(e) {
    return this.subModels.get(e);
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
    return t || this._log.printErrorExit("PRODUCT_NAME_NOT_FOUND_IN_PROFILE", [e, this._profilePath]), t.originCompatibleSdkVersion;
  }
  getProductApiMeta(e) {
    var t;
    return null === (t = this.productApiMetaMap) || undefined === t ? undefined : t.get(e);
  }
  getTargetApiMetaByProduct(e) {
    var t;
    return null === (t = this.getProductApiMeta(e)) || undefined === t ? undefined : t.targetSdkVersion;
  }
  getAllModules() {
    return [...this.subModels.values()];
  }
  getAllEntryModules() {
    const e = new Set();
    return this.subModels.forEach((t, i) => {
      t.getModuleType() === module_type_enum_js_1.ModuleType.Entry && e.add(i);
    }), e;
  }
  getTargetApplyProducts(e, t) {
    const i = (0, array_util_js_1.getElementFromArr)(this._profileOptions.modules, e);
    if (undefined === i) return;
    const r = (0, array_util_js_1.getElementFromArr)(i.targets, t);
    return undefined !== r ? r.applyToProducts : undefined;
  }
  getModuleProfileOpt(e) {
    return (0, array_util_js_1.getElementFromArr)(this._profileOptions.modules, e);
  }
  getModuleSpecificTargets() {
    return this._moduleSpecificTargets;
  }
  getParameterFileAbsolutePath() {
    return this.parameterFileAbsolutePath;
  }
  getParameterFileObj() {
    return this.parameterFileObj;
  }
  projectBuildProfileCheck(e) {
    const t = common_const_js_1.BuildProfileSchemaFileConst.PROJECT_BUILD_PROFILE_SCHEMA_PATH,
      i = common_const_js_1.BuildProfileSchemaFileConst.HAP_MODULE_BUILD_PROFILE_SCHEMA_PATH;
    fse.existsSync(e) || this._log.printErrorExit("PROJECT_BUILD_PROFILE_NOT_FOUND", [e], [[this.name]]), validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.name,
      filePath: e,
      schemaPath: i,
      checkOnly: true
    }) && this._log.printErrorExit("PROJECT_BUILD_PROFILE_NOT_COMPLY_WITH_SCHEMA", [], [[this.name], [path_1.resolve(this.projectPath, common_const_js_1.CommonConst.BUILD_FILE_NAME)]]), validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.name,
      filePath: e,
      schemaPath: t
    }), undefined === project_build_profile_loader_js_1.projectBuildProfileLoader.getBuildProfile().app.products && this._log.printErrorExit("PRODUCT_NOT_FOUND", [this._profilePath], [[this._profilePath]]);
  }
  isOhpmProject() {
    return this.getCompileApiMetaByProduct((0, find_target_product_js_1.findTargetProduct)(this).name).version >= 9 && (fse.existsSync(this.getOhPackageJson5Path()) || !fse.existsSync(this.getPackageJsonPath()));
  }
  getRemoteHspPath() {
    return path_1.resolve(this.getProjectDir(), common_const_js_1.CommonConst.OH_MODULES, build_directory_const_js_1.BuildArtifactExtension.DOT_HSP);
  }
  getCacheRemoteHspPath(e) {
    const t = path_1.resolve(inject_util_js_1.InjectUtil.getBuildCacheParentDir(this.getProjectDir(), hvigor_1.hvigorCore.getProject().getName()), build_directory_const_js_1.BuildDirConst.BUILD_ROOT);
    return path_1.resolve(t, common_const_js_1.CommonConst.CACHE, e, common_const_js_1.CommonConst.REMOTE_HSP);
  }
  getCacheIntegratedHspPath(e) {
    const t = path_1.resolve(inject_util_js_1.InjectUtil.getBuildCacheParentDir(this.getProjectDir(), hvigor_1.hvigorCore.getProject().getName()), build_directory_const_js_1.BuildDirConst.BUILD_ROOT);
    return path_1.resolve(t, common_const_js_1.CommonConst.CACHE, e, build_directory_const_js_1.BuildDirConst.INTEGRATED_HSP);
  }
  isFaMode() {
    return false;
  }
  isCrossplatformProject() {
    return this.arkuiProjectModel.isCrossplatformProject();
  }
  getArkUIXConfigJsonPath() {
    return this.arkuiProjectModel.getArkUIXConfigJsonPath();
  }
  getArkUIXConfigJsonObj() {
    return this.arkuiProjectModel.getArkUIXConfigJsonObj();
  }
}
