/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/hmos-sdk-loader.js
 */
import path_1 from "path";
import * as hos_sdkmanager_common_1 from "@ohos/hos-sdkmanager-common";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as common_const_js_1 from "../const/common-const";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as meta_util_js_1 from "../utils/meta-util";
import * as default_progress_js_1 from "./lib/default-progress";
import * as property_get_js_1 from "./lib/property-get";
import * as sdk_util_js_1 from "./sdk-util";
const _log = ohos_logger_js_1.OhosLogger.getLogger("SdkLoader");
export class HmosSdkLoader {
  constructor() {
    this.hmsSdkMap = new Map(), this.sdkMap = new Map(), this.property = new property_get_js_1.Property(), this.setHosMetaCompileSdkVersion();
  }
  static getInstance() {
    return HmosSdkLoader.container || (HmosSdkLoader.container = new HmosSdkLoader()), HmosSdkLoader.container;
  }
  setHosMetaCompileSdkVersion() {
    const e = hos_sdkmanager_common_1.HosVersionMapper.INSTANCE,
      o = e.getLatestSupportVersion(),
      s = `${e.getHosVersionByFullBaseApi(o.getFullBaseApi().getValue())}(${o.getFullBaseApi().getValue()})`;
    (0, meta_util_js_1.updateMetaCompileSdkVersion)(s);
  }
  async getHmosSdkComponents(e, o) {
    var s;
    this.validateCache();
    const t = null !== (s = this.sdkMap.get(e.fullVersion)) && undefined !== s ? s : new Map();
    if (this.checkComponentExistence(o, t, false)) {
      const e = new Map();
      return o.forEach(o => e.set(o, t.get(o))), e;
    }
    this.ohosSdkInfoHandler || this.initHandler();
    const r = hvigor_1.MetricFactory.createDurationEvent("harmonyOS sdk scan", "HarmonyOS sdk scan", hvigor_1.HvigorTaskGroupType.OTHER_TASK_GROUP);
    r.log.debug(`Local scan or download HarmonyOS sdk components ${o.join(",")}`);
    const n = this.ohosSdkInfoHandler.getLocalSdks(`${e.fullVersion}`);
    this.checkComponentExistence(o, n, false) || _log.printErrorExit("SDK_COMPONENT_MISSING");
    for (const [e, o] of n.entries()) t.set(e, o);
    this.sdkMap.set(e.fullVersion, t), r.stop();
    const a = new Map();
    return o.forEach(e => a.set(e, t.get(e))), a;
  }
  checkComponentExistence(e, o, s) {
    var t;
    for (const r of e) {
      const e = o.get(r);
      if (!e) return false;
      const n = s ? common_const_js_1.CommonConst.HMS_SDK_COMPONENT_DEFINITION : common_const_js_1.CommonConst.OHOS_SDK_COMPONENT_DEFINITION,
        a = path_1.resolve(null !== (t = e.getLocation()) && undefined !== t ? t : "", n);
      if (!fs_extra_1.existsSync(a)) return false;
    }
    return true;
  }
  async getHmsSdkComponents(e, o) {
    var s;
    this.validateCache();
    const t = null !== (s = this.hmsSdkMap.get(e.api)) && undefined !== s ? s : new Map();
    if (this.checkComponentExistence(o, t, true)) {
      const e = new Map();
      return o.forEach(o => e.set(o, t.get(o))), e;
    }
    this.hmosSdkInfoHandler || this.initHandler();
    const r = hvigor_1.MetricFactory.createDurationEvent("hmscore sdk scan", "Hmscore sdk scan", hvigor_1.HvigorTaskGroupType.OTHER_TASK_GROUP);
    r.log.debug(`Local scan or download hmscore sdk components ${o.join(",")}`);
    const n = this.hmosSdkInfoHandler.getLocalSdks(e.api);
    this.checkComponentExistence(o, n, true) || _log.printErrorExit("SDK_COMPONENT_MISSING");
    for (const [e, o] of n.entries()) t.set(e, o);
    this.hmsSdkMap.set(e.api, t), r.stop();
    const a = new Map();
    return o.forEach(e => a.set(e, t.get(e))), a;
  }
  initHandler() {
    const e = hos_sdkmanager_common_1.HosPrjSdkConfig.builder(this.property.getHosSdkDir()).sdkProxy(sdk_util_js_1.proxyFun),
      o = new hos_sdkmanager_common_1.SimpleHosPrjSdkHandler(e, new default_progress_js_1.DefaultProgress());
    this.hmosSdkInfoHandler = o.getSdkHandler(hos_sdkmanager_common_1.HosPrjSdkType.HARMONYOS), this.ohosSdkInfoHandler = o.getSdkHandler(hos_sdkmanager_common_1.HosPrjSdkType.OPENHARMONY);
  }
  validateCache() {
    const e = new property_get_js_1.Property();
    e.getHosSdkDir() !== this.property.getHosSdkDir() && (_log.debug("HarmonyOS sdk dir changed."), this.property = e, this.sdkMap.clear(), this.hmsSdkMap.clear(), this.initHandler());
  }
}
