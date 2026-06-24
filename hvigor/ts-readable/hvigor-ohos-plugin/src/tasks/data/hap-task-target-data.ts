/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/data/hap-task-target-data.js
 */
import path_1 from "path";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as device_type_const_js_1 from "../../const/device-type-const";
import * as runtime_type_enum_js_1 from "../../enum/runtime-type-enum";
import * as ohos_plugin_id_js_1 from "../../plugin/common/ohos-plugin-id";
import * as hap_extra_info_js_1 from "../../project/data/hap-extra-info";
import * as array_util_js_1 from "../../utils/array-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as status_js_1 from "../service/status";
import fs_extra_1 from "fs-extra";
import * as json_util_js_1 from "../../utils/json-util";
import * as module_task_service_js_1 from "../service/module-task-service";
import * as sdk_const_js_1 from "../../const/sdk-const";
export class ModuleTargetData {
  constructor(e, t, i, s) {
    var r;
    this._log = ohos_logger_js_1.OhosLogger.getLogger("Target"), this.compileApiVersion = e.getCompileApiMetaByProduct(s.name).version, this.compatibleApiVersion = e.getCompatibleApiMetaByProduct(s.name).version, this.targetSdkVersion = null === (r = e.getTargetApiMetaByProduct(s.name)) || undefined === r ? undefined : r.version, this.originCompatibleSdkVersion = e.getOriginCompatibleSdkVersionByProduct(s.name), this.compatibleApiVersion === sdk_const_js_1.ApiVersion.API_VERSION_12 && (this.compatibleSdkVersionStage = s.compatibleSdkVersionStage || "beta1");
    const o = e.getProductApiMeta(s.name);
    if (!o) throw new Error(`Can not find product ${s}.`);
    this._apiMeta = o, this._moduleModel = e, this._target = t, this._pathInfo = i, this._product = s, s.runtimeOS ? this._targetStatus = new status_js_1.Status(s) : (t.runtimeOS || (this._targetStatus = new status_js_1.Status({
      runtimeOS: "OpenHarmony"
    })), this._targetStatus = new status_js_1.Status(t)), this._moduleSourceSetModel = this._moduleModel.getSourceSetByTargetName(t.name);
  }
  isHarmonyOS() {
    var e, t;
    return null !== (t = null === (e = this._targetStatus) || undefined === e ? undefined : e.is(runtime_type_enum_js_1.RuntimeTypeEnum.HARMONY_OS)) && undefined !== t && t;
  }
  getApiMeta() {
    return this._apiMeta;
  }
  getCompileApiVersion() {
    return this.compileApiVersion;
  }
  getCompatibleApiVersion() {
    return this.compatibleApiVersion;
  }
  getOriginCompatibleSdkVersion() {
    return this.originCompatibleSdkVersion;
  }
  getTargetSdkVersion() {
    return this.targetSdkVersion;
  }
  getCompatibleSdkVersionStage() {
    return this.compatibleSdkVersionStage;
  }
  getModuleModel() {
    return this._moduleModel;
  }
  getModuleSourceSetModel() {
    return this._moduleSourceSetModel;
  }
  getRelatedEntryModules() {
    return this.getModuleModel().getRelatedEntryModules();
  }
  getTargetName() {
    return this._target.name;
  }
  getTargetOpt() {
    return this._target;
  }
  getPathInfo() {
    return this._pathInfo;
  }
  getModuleTargetOutputFileName(e, t, i = build_directory_const_js_1.BuildArtifactExtension.DOT_HAP, s, r) {
    var o;
    const n = this._moduleModel.getName(),
      _ = this._moduleModel.getModuleType(),
      a = this._moduleModel.getRelatedEntryModules(),
      u = "" === e || !module_task_service_js_1.ModuleTaskService.checkEntryModules(_, a) ? "" : `-${e}`,
      l = null != r ? r : this._target.name;
    let d = "";
    if (s === device_type_const_js_1.DeviceTypeConst.LITE && (d = `-${s}`), null === t) return `${n}${u}-${l}${d}${i}`;
    const g = t ? "signed" : "unsigned",
      c = null === (o = this._target.output) || undefined === o ? undefined : o.artifactName;
    return c ? `${c}${t ? "" : "-unsigned"}${i}` : `${n}${u}-${l}${d}-${g}${i}`;
  }
  getApkName(e = false) {
    const t = this._moduleModel.getModuleType(this.getTargetName());
    return e ? `${this._moduleModel.getName()}-${this._target.name}-cut-${t}.apk` : `${this._moduleModel.getName()}-${this._target.name}-${t}.apk`;
  }
  getTargetStatus() {
    return this._targetStatus;
  }
  getProduct() {
    return this._product;
  }
  findTargetDataByName(e, t) {
    var i;
    const s = this._moduleModel.getModule().findModuleByName(e);
    if (undefined === s) return;
    let r = s.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_HAP_PLUGIN);
    return undefined === r && (r = s.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_HSP_PLUGIN)), null === (i = r.getTaskService()) || undefined === i ? undefined : i.findTargetDataByName(t || this.getTargetName());
  }
  findRelatedTargetData(e) {
    var t, i;
    const s = this._moduleModel.getModule().findModuleByName(e);
    if (undefined === s || this._moduleModel.isHspModule()) return;
    const r = s.getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_HAP_PLUGIN),
      o = null === (t = r.getTaskService()) || undefined === t ? undefined : t.findTargetDataByName(this.getTargetName());
    return null != o ? o : null === (i = r.getTaskService()) || undefined === i ? undefined : i.findTargetDataByName(common_const_js_1.DefaultTargetConst.DEFAULT_TARGET);
  }
  findTargetConfigOpt(e) {
    var t;
    return (0, array_util_js_1.getElementFromArr)(null === (t = this.getTargetOpt().source) || undefined === t ? undefined : t.abilities, e.getName());
  }
  getTargetDeviceType() {
    var e;
    const t = null === (e = this.getTargetOpt().config) || undefined === e ? undefined : e.deviceType;
    return undefined === t || 0 === t.length ? this._moduleModel.getDeviceTypes() : (t.every(e => this._moduleModel.getDeviceTypes().includes(e)) || ohos_logger_js_1.OhosLogger.getLogger("target").printErrorExit("INVALID_TARGET_DEVICETYPE", [this._moduleModel.getProfilePath()], [[this.getTargetName()]]), t);
  }
  hasRichDeviceInTarget() {
    return this.getTargetDeviceType().some(e => device_type_const_js_1.DeviceTypeConst.RICH_DEVICES.includes(e));
  }
  hasLiteDeviceInTarget() {
    return this.getTargetDeviceType().some(e => device_type_const_js_1.DeviceTypeConst.LITE_DEVICES.includes(e));
  }
  isMixedDeviceTarget() {
    return this.getTargetDeviceType().some(e => device_type_const_js_1.DeviceTypeConst.LITE_DEVICES.includes(e)) && this.getTargetDeviceType().some(e => device_type_const_js_1.DeviceTypeConst.RICH_DEVICES.includes(e));
  }
  isPurerRichDeviceTarget() {
    return this.getTargetDeviceType().every(e => device_type_const_js_1.DeviceTypeConst.RICH_DEVICES.includes(e));
  }
  isPurerLiteDeviceTarget() {
    return this.getTargetDeviceType().every(e => device_type_const_js_1.DeviceTypeConst.LITE_DEVICES.includes(e));
  }
  isPurerNDeviceTarget() {
    const e = this._pathInfo.getSyscapJsonPathInfo(this._moduleModel);
    if (!fs_extra_1.existsSync(e)) return false;
    const t = json_util_js_1.JsonUtil.getJson5Obj(e).devices;
    if (!t) return false;
    const i = t.general;
    return undefined === i || 0 === i.length;
  }
  getTargetDeviceTypeClasses() {
    var e;
    const t = [];
    if (undefined === this.getTargetDeviceType()) {
      const t = (null !== (e = this._moduleModel.getProfileOpt().apiType) && undefined !== e ? e : hap_extra_info_js_1.ApiType.STAGE) === hap_extra_info_js_1.ApiType.FA ? common_const_js_1.CommonConst.CONFIG_JSON : common_const_js_1.CommonConst.MODULE_JSON5,
        i = path_1.resolve(this._moduleSourceSetModel.getSourceSetRoot(), t);
      this._log.printErrorExit("DEVICETYPE_IS_NOT_CONFIGURED", [i]);
    }
    return this.hasLiteDeviceInTarget() && t.push(device_type_const_js_1.DeviceTypeConst.LITE), this.hasRichDeviceInTarget() && t.push(device_type_const_js_1.DeviceTypeConst.RICH), this.isPurerNDeviceTarget() && t.push(device_type_const_js_1.DeviceTypeConst.RICH), t;
  }
  isSingleDeviceTypeTarget() {
    return this.isPurerRichDeviceTarget() || this.isPurerLiteDeviceTarget();
  }
}
