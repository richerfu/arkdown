/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/one-sdk-validator.js
 */
import * as hos_sdkmanager_common_1 from "@ohos/hos-sdkmanager-common";
import * as sdkmanager_common_1 from "@ohos/sdkmanager-common";
import * as find_target_product_js_1 from "../common/find-target-product";
import * as common_const_js_1 from "../const/common-const";
import * as sdk_const_js_1 from "../const/sdk-const";
import * as project_build_profile_js_1 from "../options/build/project-build-profile";
import * as ohos_logger_js_1 from "./log/ohos-logger";
import * as validate_util_js_1 from "./validate/validate-util";
import * as task_util_js_1 from "./task-util";
import * as hvigor_1 from "@ohos/hvigor";
var ApiValType = project_build_profile_js_1.ProjectBuildProfile.ApiValType;
export class OneSdkValidator {
  static evlProductRuntimeOS(o) {
    const t = (0, find_target_product_js_1.findTargetProduct)(o),
      e = o.getTargetRuntimeOSs();
    if (t.runtimeOS) return t.runtimeOS;
    return e.filter(o => o.productName === t.name).find(o => o.runtimeOS === common_const_js_1.CommonConst.HARMONY_OS) ? common_const_js_1.CommonConst.HARMONY_OS : common_const_js_1.CommonConst.OPEN_HARMONY;
  }
  static apiInspection(o) {
    var t, e, i;
    const r = (0, find_target_product_js_1.findTargetProduct)(o),
      s = this.evlProductRuntimeOS(o) === common_const_js_1.CommonConst.HARMONY_OS,
      n = (0, task_util_js_1.isAtomicServiceProject)(r, o);
    validate_util_js_1.ValidateUtil.integrationVersionCheck(s, (0, find_target_product_js_1.findTargetProduct)(o), n, o.getProfilePath());
    const _ = null === (t = o.getProductApiMeta(r.name)) || undefined === t ? undefined : t.compileSdkVersion,
      a = null === (e = o.getProductApiMeta(r.name)) || undefined === e ? undefined : e.compatibleSdkVersion,
      p = null === (i = o.getProductApiMeta(r.name)) || undefined === i ? undefined : i.targetSdkVersion;
    this.versionCompatibilityCheck(o, s, common_const_js_1.CommonConst.COMPILE_SDK_VERSION, _), this.versionCompatibilityCheck(o, s, common_const_js_1.CommonConst.COMPATIBLE_SDK_VERSION, a), this.versionCompatibilityCheck(o, s, common_const_js_1.CommonConst.TARGET_SDK_VERSION, p), this.versionTypeCheck(o, s, _), this.versionTypeCheck(o, s, a), this.versionTypeCheck(o, s, p);
    const l = _.version,
      c = a.version,
      d = null == p ? undefined : p.version;
    if (l < 8 && this._log.printErrorExit("SDK_VERSION_UNDER_API8", [l, o.getProfilePath()]), s && _.version > 9 && (0 === _.type || 0 === a.type || p && 0 === p.type)) {
      const t = s ? "HO_API_VERSION_VALIDATE_FAILED" : "OH_API_VERSION_VALIDATE_FAILED";
      this._log.printErrorExit(t, undefined, [[o.getProfilePath()]]);
    }
    const m = [r.compileSdkVersion, r.compatibleSdkVersion, r.targetSdkVersion];
    s ? m.forEach(t => this.hoApiTypeCheck(t, o)) : m.forEach(t => this.ohApiTypeCheck(t, o)), this.compatibleVersionCheck(s, a, o), s && OneSdkValidator.evlApiModel(o, r) === ApiModel.ONE_MSF && this.msfCompare(o, _, a, p), (d ? c <= d && c <= l && d <= l : c <= l) || this._log.printErrorExit("SDKVERSION_ORDER_ERROR", [o.getProfilePath()]);
  }
  static compareModelVersions(o, t) {
    const e = o.split(".").map(o => {
        var t;
        return Number.isNaN(parseInt(o)) ? 0 : null !== (t = parseInt(o)) && undefined !== t ? t : 0;
      }),
      i = t.split(".").map(o => {
        var t;
        return Number.isNaN(parseInt(o)) ? 0 : null !== (t = parseInt(o)) && undefined !== t ? t : 0;
      });
    for (let o = 0; o < 3; o++) {
      const t = e[o],
        r = i[o];
      if (t < r) return -1;
      if (t > r) return 1;
    }
    return 0;
  }
  static compatibleVersionCheck(o, t, e) {
    if (0 !== t.minor || 0 !== t.patch) {
      const t = o ? "HO_API_VERSION_VALIDATE_FAILED" : "OH_API_VERSION_VALIDATE_FAILED";
      this._log.printErrorExit(t, undefined, [[e.getProfilePath()]]);
    }
  }
  static ohApiTypeCheck(o, t) {
    var e;
    const i = null === (e = hvigor_1.HvigorConfigLoader.getInstance().getHvigorConfig()) || undefined === e ? undefined : e.modelVersion;
    i && OneSdkValidator.compareModelVersions(i, "6.0.0") < 0 && o && "number" != typeof o && this._log.printErrorExit("OH_API_VERSION_VALIDATE_FAILED", undefined, [[t.getProfilePath()]]);
  }
  static hoApiTypeCheck(o, t) {
    o && "number" == typeof o && this._log.printErrorExit("HO_API_VERSION_VALIDATE_FAILED", undefined, [[t.getProfilePath()]]);
  }
  static versionTypeCheck(o, t, e) {
    if (!t && (null == e ? undefined : e.type) === ApiValType.NUM) return;
    if (!e) return;
    const i = e.version;
    if (e.type === ApiValType.NUM ? i > sdk_const_js_1.ApiVersion.API_VERSION_9 : i < sdk_const_js_1.ApiVersion.API_VERSION_9) {
      e.type, ApiValType.NUM;
      const i = t ? "HO_API_VERSION_VALIDATE_FAILED" : "OH_API_VERSION_VALIDATE_FAILED";
      this._log.printErrorExit(i, undefined, [[o.getProfilePath()]]);
    }
  }
  static versionCompatibilityCheck(o, t, e, i) {
    if (!t || (null == i ? undefined : i.type) === ApiValType.NUM) return;
    if (!i) return;
    const r = hos_sdkmanager_common_1.HosVersionMapper.INSTANCE.transferVersionIntoHosVersion(i.api);
    r ? this.isBaseApiEqual(r, i) || this._log.printErrorExit("SDKMANAGER_APIVERSION_AND_PLATFORM_NOT_MATCH", [i.api, i.version, e, o.getProfilePath()]) : this._log.printErrorExit("SDKMANAGER_NOT_SUPPORT_THIS_API", [e, i.api, i.version, o.getProfilePath()]);
  }
  static isBaseApiEqual(o, t) {
    const e = new sdkmanager_common_1.ApiVersion(t.fullVersion);
    return (0, sdkmanager_common_1.isEqualApiVersion)(o.getFullBaseApi(), e);
  }
  static msfCompare(o, t, e, i) {
    let r = OneSdkValidator.compareMsf(t.api, e.api, o);
    r < 0 && this._log.printErrorExit("SDKVERSION_ORDER_ERROR", [o.getProfilePath()]), i && (r = OneSdkValidator.compareMsf(t.api, i.api, o), r < 0 && this._log.printErrorExit("SDKVERSION_ORDER_ERROR", [o.getProfilePath()]), r = OneSdkValidator.compareMsf(i.api, e.api, o), r < 0 && this._log.printErrorExit("SDKVERSION_ORDER_ERROR", [o.getProfilePath()]));
  }
  static compareMsf(o, t, e) {
    const i = OneSdkValidator.parseMsf(o, e),
      r = OneSdkValidator.parseMsf(t, e);
    return i.m !== r.m ? i.m > r.m ? 1 : -1 : i.s !== r.s ? i.s > r.s ? 1 : -1 : i.f !== r.f ? i.f > r.f ? 1 : -1 : 0;
  }
  static parseMsf(o, t) {
    const e = o.split(".");
    3 !== e.length && this._log.printErrorExit("HO_API_VERSION_VALIDATE_FAILED", undefined, [[t.getProfilePath()]]);
    return {
      m: parseInt(e[0]),
      s: parseInt(e[1]),
      f: parseInt(e[2])
    };
  }
  static evlApiModel(o, t) {
    var e, i;
    const r = null === (e = o.getProductApiMeta(t.name)) || undefined === e ? undefined : e.compileSdkVersion,
      s = null === (i = o.getProductApiMeta(t.name)) || undefined === i ? undefined : i.compatibleSdkVersion;
    return 1 === r.type && 1 === s.type ? ApiModel.ONE_MSF : 0 === r.type && 0 === s.type ? ApiModel.ONE_NUM : ApiModel.ORIGINAL;
  }
}
export var ApiModel;
OneSdkValidator._log = ohos_logger_js_1.OhosLogger.getLogger("OneSdk"), function (o) {
  o[o.ORIGINAL = 0] = "ORIGINAL", o[o.ONE_MSF = 1] = "ONE_MSF", o[o.ONE_NUM = 2] = "ONE_NUM", o[o.UNDEFINED = 4] = "UNDEFINED";
}(ApiModel = ApiModel || (ApiModel = {}));
