/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/sdk-loader.js
 */
import * as sdk_const_js_1 from "../const/sdk-const";
import * as hos_sdk_info_js_1 from "./info/hos-sdk-info";
import * as ohos_sdk_info_js_1 from "./info/ohos-sdk-info";
import * as property_get_js_1 from "./lib/property-get";
export class SdkLoader {
  constructor(s, o = false) {
    this.isOhos = false, this.isOhos = o, this.apiMeta = s;
  }
  static getSdkInfo(s, o = false) {
    if (this.loader || (this.loader = new SdkLoader(s, o)), this.loader.sdkInfo && this.loader.isOhos === o && this.apiCompare(s, this.loader.apiMeta)) return this.loader.sdkInfo;
    const e = new property_get_js_1.Property(!this.loader.isOhos),
      t = this.loader.isOhos ? e.getSdkDir() : e.getHosSdkDir(),
      r = o ? new ohos_sdk_info_js_1.OhosSdkInfo(sdk_const_js_1.SDK_COMPONENTS, s, t) : new hos_sdk_info_js_1.HosSdkInfo(sdk_const_js_1.SDK_COMPONENTS, s, t);
    return this.loader.sdkInfo = r, this.loader.isOhos = o, this.loader.apiMeta = s, r;
  }
  static apiCompare(s, o) {
    return s.api === o.api && s.version === o.version && s.type === o.type;
  }
}
