/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/ohos-sdk-loader.js
 */
import * as sdkmanager_common_1 from "@ohos/sdkmanager-common";
import * as path_and_api_map_js_1 from "@ohos/sdkmanager-common/build/src/core/util/path-and-api-map";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as common_const_js_1 from "../const/common-const";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as default_progress_js_1 from "./lib/default-progress";
import * as property_get_js_1 from "./lib/property-get";
import * as sdk_util_js_1 from "./sdk-util";
const _log = ohos_logger_js_1.OhosLogger.getLogger("SdkLoader");
export class OhosSdkLoader {
  constructor() {
    this.sdkMap = new path_and_api_map_js_1.PathAndApiMap(), this.property = new property_get_js_1.Property(false);
  }
  static getInstance() {
    return OhosSdkLoader.container || (OhosSdkLoader.container = new OhosSdkLoader()), OhosSdkLoader.container;
  }
  async getOhosSdkComponents(e, s) {
    this.validateCache();
    const r = [],
      t = new sdkmanager_common_1.ApiVersion(e.fullVersion);
    s.forEach(e => r.push(new sdkmanager_common_1.PathAndApiVersion(e, t)));
    const o = this.pickRequired(this.sdkMap, r),
      a = [...o.values()].filter(e => {
        var s;
        return !fs_extra_1.existsSync(path_1.resolve(null !== (s = e.getLocation()) && undefined !== s ? s : "", common_const_js_1.CommonConst.OHOS_SDK_COMPONENT_DEFINITION));
      });
    if (s.length === o.size && 0 === a.length) return o;
    const i = (0, sdk_util_js_1.ohosPredict)(this.property.getSdkDir(), e.api, s);
    0 === i.size && this.sdkMap.clear();
    for (const [e, s] of i.entries()) this.sdkMap.has(e) || this.sdkMap.set(e, s);
    const n = this.pickRequired(this.sdkMap, r);
    if (s.length === n.size) return n;
    this.sdkInfoHandler || this.initHandler();
    return (await this.sdkInfoHandler.getOrDownload(r)).forEach((e, s) => {
      this.sdkMap.has(s) || this.sdkMap.set(s, e);
    }), this.pickRequired(this.sdkMap, r);
  }
  pickRequired(e, s) {
    const r = new Map();
    return s.forEach(s => {
      const t = e.get(s);
      t && r.set(s, t);
    }), r;
  }
  initHandler() {
    const e = sdkmanager_common_1.OhPrjSdkConfig.builder(this.property.getSdkDir()).sdkProxy(sdk_util_js_1.proxyFun),
      s = new sdkmanager_common_1.SimpleOhPrjSdkHandler(e, new default_progress_js_1.DefaultProgress());
    this.sdkInfoHandler = s.getSdkHandler(sdkmanager_common_1.OhPrjSdkType.OPENHARMONY);
  }
  validateCache() {
    const e = new property_get_js_1.Property(false);
    e.getSdkDir() !== this.property.getSdkDir() && (_log.debug("HarmonyOS sdk dir changed."), this.property = e, this.sdkMap.clear(), this.initHandler());
  }
}
