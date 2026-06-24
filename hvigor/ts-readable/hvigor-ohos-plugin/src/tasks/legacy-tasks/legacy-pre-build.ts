/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-pre-build.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import fs_1 from "fs";
import path_1 from "path";
import * as common_const_js_1 from "../../const/common-const";
import * as error_handlers_js_1 from "../../utils/validate/error-handlers";
import * as validate_util_js_1 from "../../utils/validate/validate-util";
import * as abstract_pre_build_js_1 from "../abstract/abstract-pre-build";
import * as task_names_js_1 from "../common/task-names";
import fs_extra_1 from "fs-extra";
import * as file_util_js_1 from "../../utils/file-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as meta_util_js_1 from "../../utils/meta-util";
import * as build_option_path_info_js_1 from "../../common/build-option-path-info";
var __decorate = this && this.__decorate || function (e, t, o, i) {
  var a,
    s = arguments.length,
    r = s < 3 ? t : null === i ? i = Object.getOwnPropertyDescriptor(t, o) : i;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) r = Reflect.decorate(e, t, o, i);else for (var l = e.length - 1; l >= 0; l--) (a = e[l]) && (r = (s < 3 ? a(r) : s > 3 ? a(t, o, r) : a(t, o)) || r);
  return s > 3 && r && Object.defineProperty(t, o, r), r;
};
var LegacyFATask = task_names_js_1.TaskNames.LegacyFATask;
var CommonTask = task_names_js_1.TaskNames.CommonTask;
const _log = ohos_logger_js_1.OhosLogger.getLogger("hvigor-legacyPreBuild");
export class LegacyPreBuild extends abstract_pre_build_js_1.AbstractPreBuild {
  get isExpandImportEnabled() {
    var e, t;
    return !!(null === (t = null === (e = this.targetService.getBuildOption().arkOptions) || undefined === e ? undefined : e.expandImportPath) || undefined === t ? undefined : t.enable);
  }
  declareInputFiles() {
    var e;
    const t = new hvigor_1.FileSet().addEntries([this.targetJsonPath], {
      isDirectory: false
    });
    fs_1.existsSync(path_1.resolve(null === (e = this.moduleModel) || undefined === e ? undefined : e.getSourceRootByTargetName(this.targetName), common_const_js_1.CommonConst.SYSCAP_JSON)) && t.addEntry(this.syscapJsonPath);
    const o = this.isOhpmProject ? this.moduleModel.getOhPackageJson5Path() : this.moduleModel.getPackageJsonPath();
    return fs_1.existsSync(o) && t.addEntry(o), fs_1.existsSync(this.projectProfilePath) && t.addEntry(this.projectProfilePath), fs_1.existsSync(this.profilePath) && t.addEntry(this.profilePath), t;
  }
  constructor(e) {
    super(e, LegacyFATask.PRE_BUILD), this.logger = ohos_logger_js_1.OhosLogger.getLogger(LegacyPreBuild.name);
    const t = e.getTargetData().getModuleSourceSetModel().getLegacyModuleTargetRes();
    this.targetJsonPath = t.getJsonPath(), this.targetConfigOptObj = t.getConfigJsonOpt(), this.configOptObj = fs_extra_1.readJsonSync(this.targetJsonPath);
  }
  doValidateForDiffApiType() {
    const e = this.targetName,
      t = this.service.hasLiteDevice() ? this.sdkInfo.getLiteSchema() : this.sdkInfo.getRichSchema(),
      o = this.targetJsonPath;
    validate_util_js_1.ValidateUtil.submitSchemaCheckWork({
      moduleName: this.moduleName,
      filePath: o,
      schemaPath: t,
      errorHandlers: [error_handlers_js_1.removeLegacyPhoneSupport]
    }), this.validateAbilities(), this.validateForm(), e !== common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET && this.validateModuleName(this.moduleName, this.targetConfigOptObj.module.distro.moduleName, common_const_js_1.CommonConst.CONFIG_JSON), validate_util_js_1.ValidateUtil.validateFALiteDevice(this.configOptObj.module.deviceType), validate_util_js_1.ValidateUtil.validateHybridDeviceTypes(this.configOptObj.module.deviceType);
  }
  getJsonProfileByModel() {
    return {
      jsonFilePath: this.targetJsonPath,
      profile: this.configOptObj,
      deviceTypes: this.configOptObj.module.deviceType,
      deviceConfig: common_const_js_1.CommonConst.DEVICE_TYPE,
      configurationProfile: common_const_js_1.CommonConst.CONFIG_JSON
    };
  }
  validateForm() {
    var e, t;
    null === (t = null === (e = this.targetConfigOptObj) || undefined === e ? undefined : e.module.js) || undefined === t || t.forEach(e => {
      var t;
      if ("form" !== e.type) return;
      const o = this.moduleModel.getProjectDir();
      null === (t = e.pages) || undefined === t || t.forEach(t => {
        const i = file_util_js_1.FileUtil.convertToAbsolutePath(t, path_1.resolve(o, "src/main/js", e.name));
        this.isValidWidget(false, "", i) || this.logger.printErrorExit("FORM_PAGES_NOT_FOUND", [t, this.targetJsonPath], [[t], [t]]);
      });
    });
  }
  validateJsType(e) {
    var t, o;
    null === (o = null === (t = this.targetConfigOptObj) || undefined === t ? undefined : t.module.js) || undefined === o || o.forEach(t => {
      var o;
      if (t.name === e) switch (t.type) {
        case undefined:
          "form" === (null === (o = t.mode) || undefined === o ? undefined : o.type) && this.logger.printErrorExit("SAME_NAME_BETWEEN_FORM_AND_ABILITY", [e, "module.js.mode", this.targetJsonPath]);
          break;
        case "form":
          this.logger.printErrorExit("SAME_NAME_BETWEEN_FORM_AND_ABILITY", [e, "module.js", this.targetJsonPath]);
      }
    });
  }
  validateAbilities() {
    var e, t;
    null === (t = null === (e = this.targetConfigOptObj) || undefined === e ? undefined : e.module.abilities) || undefined === t || t.forEach(e => {
      if ("page" === e.type) {
        const t = e.name;
        this.validateJsType(t);
      }
    });
  }
  validateAtomicService() {
    this.isAtomicServiceProject() && _log.printErrorExit("ATOMIC_SERVICE_DOES_NOT_SUPPORT_IN_FA");
  }
  initTaskDepends() {
    (0, meta_util_js_1.hvigorOrToolChainsChanged)(this.targetService.getSdkInfo()) && this.dependsOn(CommonTask.CLEAN.name);
  }
  checkExtendSourceDirs() {}
  checkByteCodeHar() {}
  validatePreloadSystemSo() {
    super.validatePreloadSystemSo(), this.isPreloadSystemSoEnabled && _log.printErrorExit("FA_DOES_NOT_SUPPORT_PRELOAD_SYSTEM_SO", [], [[build_option_path_info_js_1.buildOptionPath.getTargetBuildOptPath(this.moduleModel, this.targetName, "preloadSystemSo")]]);
  }
  checkExpandImportConfigs() {
    this.isExpandImportEnabled && _log.printErrorExit("FA_DOES_NOT_SUPPORT_EXPAND_IMPORT_PATH", [], [[build_option_path_info_js_1.buildOptionPath.getTargetBuildOptPath(this.moduleModel, this.targetName, "arkOptions.expandImportPath.enable")]]);
  }
  checkFormModule() {}
}
__decorate([(0, hvigor_1.Input)()], LegacyPreBuild.prototype, "isExpandImportEnabled", null);
