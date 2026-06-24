/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/module/module-model-impl.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as find_target_product_js_1 from "../../common/find-target-product";
import * as common_const_js_1 from "../../const/common-const";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as array_util_js_1 from "../../utils/array-util";
import * as file_util_js_1 from "../../utils/file-util";
import * as form_util_js_1 from "../../utils/form-util";
import * as json_util_js_1 from "../../utils/json-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_util_js_1 from "../../utils/task-util";
import * as validate_util_js_1 from "../../utils/validate/validate-util";
import * as target_source_set_impl_js_1 from "../source-set/target-source-set-impl";
import * as core_module_model_impl_js_1 from "./core-module-model-impl";
const defaultTargetName = common_const_js_1.DefaultTargetConst.DEFAULT_TARGET,
  ohosTestTargetName = common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET,
  _log = ohos_logger_js_1.OhosLogger.getLogger("moduleModel");
export class ModuleModelImpl extends core_module_model_impl_js_1.CoreModuleModelImpl {
  constructor(e, t) {
    super(e, t), this.initDefaultTargetSourceSet();
  }
  getDeviceTypes() {
    var e;
    const t = null === (e = this.getSourceSetByTargetName().getModuleTargetRes().getModuleJsonOpt()) || undefined === e ? undefined : e.module.deviceTypes;
    return null == t && _log.printErrorExit("DEVICE_TYPE_NOT_FOUND_STAGE", [], [[this.getSourceSetByTargetName().getModuleTargetRes().getJsonPath()]]), t;
  }
  initDefaultTargetSourceSet() {
    const e = new target_source_set_impl_js_1.TargetSourceSetImpl(path_1.resolve(this._sourceRootDir, "main"));
    this.targetSourceSetMap.set(defaultTargetName, e);
    if ((0, array_util_js_1.getElementFromArr)(this.getProfileOpt().targets, ohosTestTargetName)) {
      const e = new target_source_set_impl_js_1.TargetSourceSetImpl(path_1.resolve(this._sourceRootDir, ohosTestTargetName));
      this.targetSourceSetMap.set(ohosTestTargetName, e);
    }
  }
  getModuleType() {
    const e = this.getSourceSetByTargetName();
    e || _log.printErrorExit("STAGE_NO_SYSTEM_PLUGIN", [`${path_1.resolve(this.getProjectDir(), hvigor_1.HvigorCommonConst.BUILD_FILE_NAME)}.ts`], [[`${path_1.resolve(this.getProjectDir(), hvigor_1.HvigorCommonConst.BUILD_FILE_NAME)}.ts`]]);
    const t = e.getModuleTargetRes().getModuleJsonOpt();
    return undefined === t && _log.printErrorExit("MODULEJSON5_FILE_NOT_FOUND", [], [[e.getModuleTargetRes().getJsonPath()]]), undefined === t.module && _log.printErrorExit("NO_MODULE_INFORMATION", [e.getModuleTargetRes().getJsonPath()], [[e.getModuleTargetRes().getJsonPath()]]), undefined === t.module.type && _log.printErrorExit("NO_MODULE_TYPE", [e.getModuleTargetRes().getJsonPath()], [[e.getModuleTargetRes().getJsonPath()]]), module_type_enum_js_1.ModuleType.valueOf(t.module.type);
  }
  getSourceSetByTargetName(e = defaultTargetName) {
    return this.targetSourceSetMap.has(e) ? this.targetSourceSetMap.get(e) : this.targetSourceSetMap.get(defaultTargetName);
  }
  getJsonObjByTargetName(e) {
    return this.getSourceSetByTargetName(e).getModuleTargetRes().getModuleJsonOpt();
  }
  getJsonPathByTargetName(e) {
    return this.getSourceSetByTargetName(e).getModuleTargetRes().getJsonPath();
  }
  isAtomicService() {
    const e = (0, find_target_product_js_1.findTargetProduct)(this.getParentProject());
    return (0, task_util_js_1.isAtomicServiceProject)(e, this.getParentProject());
  }
  isInstallFree() {
    return this.getJsonObjByTargetName(defaultTargetName).module.installationFree || false;
  }
  getFormJsonObjByTargetName(e, t) {
    if (this.isFormExtensionModule(e)) {
      const o = form_util_js_1.FormUtil.getFormWidgetModuleByTargetName(this, e);
      if (o) return o.getFormJsonObjByTargetName(e, t);
    }
    const o = this.getSourceSetByTargetName(e).getModuleTargetRes().getFormJsonOpt(t),
      r = path_1.resolve(this.getSourceSetByTargetName(e).getModuleTargetRes().getResourcePath(), t);
    return validate_util_js_1.ValidateUtil.validateDuplicatedName(o.forms, "Form", r), validate_util_js_1.ValidateUtil.isDefaultCheck(o.forms, r), validate_util_js_1.ValidateUtil.checkUpdateEnable(o.forms, r), o;
  }
  getPermission() {
    const e = this.getSourceSetByTargetName().getModuleTargetRes().getModuleJsonOpt();
    return {
      requestPermissions: e.module.requestPermissions,
      definePermissions: e.module.definePermissions
    };
  }
  getCompressNativeLib() {
    return this.getSourceSetByTargetName().getModuleTargetRes().getModuleJsonOpt().module.compressNativeLibs;
  }
  isFormWidgetModule(e) {
    var t;
    const o = this.getSourceSetByTargetName(e).getModuleTargetRes().getModuleJsonOpt();
    return this.isHspModule() && !!(null === (t = null == o ? undefined : o.module) || undefined === t ? undefined : t.formExtensionModule);
  }
  isFormExtensionModule(e) {
    var t;
    const o = this.getSourceSetByTargetName(e).getModuleTargetRes().getModuleJsonOpt();
    return this.isHapModule() && !!(null === (t = null == o ? undefined : o.module) || undefined === t ? undefined : t.formWidgetModule);
  }
  hasFormConfig(e) {
    const t = this.getSourceSetByTargetName(e).getTargetResPath(),
      o = path_1.resolve(t, "base", "profile"),
      r = file_util_js_1.FileUtil.getAllFilesFromFolder(o);
    for (const e of r) {
      if (json_util_js_1.JsonUtil.getJson5Obj(e).forms) return true;
    }
    return false;
  }
}
