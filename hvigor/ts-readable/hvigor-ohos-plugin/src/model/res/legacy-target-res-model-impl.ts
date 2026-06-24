/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/res/legacy-target-res-model-impl.js
 */
import path_1 from "path";
import * as common_const_js_1 from "../../const/common-const";
import * as device_type_const_1 from "../../const/device-type-const";
import * as core_res_model_impl_js_1 from "./core-res-model-impl";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as res_model_loader_js_1 from "../../utils/loader/file/res-model-loader";
export class LegacyModuleTargetResImpl extends core_res_model_impl_js_1.CoreResModelImpl {
  constructor(e) {
    super(path_1.resolve(e, common_const_js_1.CommonConst.CONFIG_JSON), path_1.resolve(e, build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES));
  }
  getConfigJsonOpt() {
    var e, o;
    if (!this._configJsonOpt) {
      if (this._configJsonOpt = res_model_loader_js_1.resModelLoader.getConfigJson(this.getJsonPath()), !this._configJsonOpt) return this._configJsonOpt;
      const t = null === (o = null === (e = this._configJsonOpt) || undefined === e ? undefined : e.module) || undefined === o ? undefined : o.abilities;
      this.hasLiteDevice() && t && t.length > 0 && (this._configJsonOpt.module.mainAbility = t[0].name);
    }
    return this._configJsonOpt;
  }
  hasLiteDevice() {
    if (!this._configJsonOpt) throw new Error(`${this.getJsonPath()} content is not obtained.`);
    return !this._configJsonOpt.module.deviceType || this._configJsonOpt.module.deviceType.some(e => "liteWearable" === e || "smartVision" === e);
  }
  supportLiteDeviceModule() {
    const e = this.getConfigJsonOpt().module.deviceType;
    return e && (e.includes(device_type_const_1.DeviceTypeConst.LITE_WEARABLE) || e.includes(device_type_const_1.DeviceTypeConst.SMART_VISION));
  }
}
