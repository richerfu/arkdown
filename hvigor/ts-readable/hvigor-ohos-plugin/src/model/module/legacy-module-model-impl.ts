/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/module/legacy-module-model-impl.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as find_target_product_js_1 from "../../common/find-target-product";
import * as common_const_js_1 from "../../const/common-const";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as build_mode_manager_js_1 from "../../project/build-option/build-mode-manager";
import * as array_util_js_1 from "../../utils/array-util";
import * as file_util_js_1 from "../../utils/file-util";
import * as res_model_loader_js_1 from "../../utils/loader/file/res-model-loader";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as legacy_ability_model_impl_js_1 from "../ability/legacy-ability-model-impl";
import * as legacy_form_model_impl_js_1 from "../ability/legacy-form-model-impl";
import * as legacy_worker_model_impl_js_1 from "../ability/legacy-worker-model-impl";
import * as legacy_target_source_set_impl_js_1 from "../source-set/legacy-target-source-set-impl";
import * as core_module_model_impl_js_1 from "./core-module-model-impl";
const defaultTargetName = common_const_js_1.DefaultTargetConst.DEFAULT_TARGET,
  ohosTestTargetName = common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET,
  _log = ohos_logger_js_1.OhosLogger.getLogger("legacyModuleModel");
export class LegacyModuleModelImpl extends core_module_model_impl_js_1.CoreModuleModelImpl {
  constructor(e, t) {
    super(e, t), this.initDefaultTargetSourceSet();
  }
  static validateSameNameAbility(e) {
    const t = ohos_logger_js_1.OhosLogger.getLogger("Ability"),
      o = (0, array_util_js_1.findDuplicateElement)(e.map(e => e.getName()));
    o.length > 0 && (o.forEach(e => t.error(`Duplicate ability name ${e}.`)), t.printErrorExit("ABILITY_NAME_IS_NOT_UNIQUE", [o.join(", ")]));
  }
  initDefaultTargetSourceSet() {
    const e = new legacy_target_source_set_impl_js_1.LegacyTargetSourceSetImpl(path_1.resolve(this._sourceRootDir, "main"));
    this.targetSourceSetMap.set(defaultTargetName, e);
    if ((0, array_util_js_1.getElementFromArr)(this.getProfileOpt().targets, ohosTestTargetName)) {
      const e = new legacy_target_source_set_impl_js_1.LegacyTargetSourceSetImpl(path_1.resolve(this._sourceRootDir, ohosTestTargetName));
      this.targetSourceSetMap.set(ohosTestTargetName, e);
    }
  }
  getLegacyAbilities(e = defaultTargetName) {
    this._legacyAbilitiesMap || (this._legacyAbilitiesMap = new Map(), this.initAbilityInfo(defaultTargetName, this.targetSourceSetMap.get(defaultTargetName)), e === ohosTestTargetName && this.initAbilityInfo(ohosTestTargetName, this.targetSourceSetMap.get(ohosTestTargetName)));
    const t = this._legacyAbilitiesMap.get(e);
    return undefined === t ? this._legacyAbilitiesMap.get("default") : t;
  }
  getModuleType(e = defaultTargetName) {
    var t, o, r;
    const a = this.getSourceSetByTargetName(e),
      i = null === (r = null === (o = null === (t = a.getLegacyModuleTargetRes().getConfigJsonOpt()) || undefined === t ? undefined : t.module) || undefined === o ? undefined : o.distro) || undefined === r ? undefined : r.moduleType;
    return undefined === i && _log.printErrorExit("FAILED_TO_OBTAIL_THE_MODULE_TYPE", [], [[a.getLegacyModuleTargetRes().getJsonPath()], [a.getLegacyModuleTargetRes().getJsonPath()]]), module_type_enum_js_1.ModuleType.valueOf(i);
  }
  getDeviceTypes() {
    const e = this.getSourceSetByTargetName().getLegacyModuleTargetRes().getConfigJsonOpt().module.deviceType;
    return null == e && _log.printErrorExit("DEVICE_TYPE_NOT_FOUND_FA", [], [[this.getSourceSetByTargetName().getLegacyModuleTargetRes().getJsonPath()]]), e;
  }
  getSourceSetByTargetName(e = defaultTargetName) {
    return this.targetSourceSetMap.has(e) ? this.targetSourceSetMap.get(e) : this.targetSourceSetMap.get(defaultTargetName);
  }
  getJsonObjByTargetName(e) {
    return this.getSourceSetByTargetName(e).getLegacyModuleTargetRes().getConfigJsonOpt();
  }
  getJsonPathByTargetName(e) {
    return this.getSourceSetByTargetName(e).getLegacyModuleTargetRes().getJsonPath();
  }
  isAtomicService() {
    var e, t;
    return this.isInstallFree() || (null === (t = null === (e = this.getJsonObjByTargetName(defaultTargetName).module) || undefined === e ? undefined : e.abilities) || undefined === t ? undefined : t.some(e => "service" === e.type && e.formsEnabled)) || false;
  }
  isInstallFree() {
    return this.getJsonObjByTargetName(defaultTargetName).module.distro.installationFree || false;
  }
  initAbilityInfo(e, t) {
    var o, r, a;
    const i = t.getLegacyModuleTargetRes().getJsonPath();
    if (!fs_1.existsSync(i)) return;
    const s = res_model_loader_js_1.resModelLoader.getConfigJson(i),
      l = s.module.abilities ? s.module.abilities : [],
      g = [];
    for (let e = 0; e < l.length; e++) g.push(new legacy_ability_model_impl_js_1.LegacyAbilityModelImpl(i, l[e].name));
    LegacyModuleModelImpl.validateSameNameAbility(g);
    const _ = s.module.js ? s.module.js : [];
    for (let e = 0; e < _.length; e++) "form" === _[e].type && g.push(new legacy_form_model_impl_js_1.LegacyFormModelImpl(i, _[e]));
    const u = (0, find_target_product_js_1.findTargetProduct)(this.getParentProject()),
      m = this.getCompileApiMetaByProduct(u.name).version;
    if (e !== common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET && m > sdk_const_js_1.ApiVersion.API_VERSION_8) {
      const t = file_util_js_1.FileUtil.convertToAbsolutePaths(null !== (a = null === (r = null === (o = build_mode_manager_js_1.buildOptionManager.getTargetBuildOption(this.getModule().getName(), e)) || undefined === o ? undefined : o.sourceOption) || undefined === r ? undefined : r.workers) && undefined !== a ? a : [], this.getProjectDir());
      t && t.filter(e => ".ts" === path_1.extname(e)).length > 0 && g.push(new legacy_worker_model_impl_js_1.LegacyWorkerModelImpl(t));
    }
    LegacyModuleModelImpl.validateFormUnique(l), this._legacyAbilitiesMap.set(e, g);
  }
  static validateFormUnique(e) {
    const t = ohos_logger_js_1.OhosLogger.getLogger("Form"),
      o = e.map(e => e.forms ? e.forms.map(e => e.name) : []).flat(),
      r = (0, array_util_js_1.findDuplicateElement)(o);
    r.length > 0 && (r.forEach(e => t.error(`Duplicate ability name ${e}.`)), t.printErrorExit("FORM_NAME_IS_NOT_UNIQUE", [r.join(", ")]));
  }
  getPermission() {
    const e = this.getSourceSetByTargetName().getLegacyModuleTargetRes().getConfigJsonOpt();
    return {
      requestPermissions: e.module.reqPermissions,
      definePermissions: e.module.defPermissions
    };
  }
  isFormWidgetModule(e) {
    return false;
  }
  isFormExtensionModule(e) {
    return false;
  }
  hasFormConfig(e) {
    return false;
  }
}
