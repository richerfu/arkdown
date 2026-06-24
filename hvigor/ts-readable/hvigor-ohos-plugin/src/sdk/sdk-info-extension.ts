/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/sdk-info-extension.js
 */
import * as component_contants_js_1 from "@ohos/sdkmanager-common/build/src/core/constants/component-contants";
import * as arkuix_sdk_info_js_1 from "./info/arkuix-sdk-info";
import * as property_get_js_1 from "./lib/property-get";
export class SdkInfoExtension {
  constructor(o) {
    this.property = new property_get_js_1.Property(false), this.sdkVersion = o;
  }
  getArkUIXInfo() {
    return this.sdkInfo = new arkuix_sdk_info_js_1.ArkUIXSdkInfo([component_contants_js_1.ComponentPath.ARKUI_X], this.sdkVersion, this.getArkUIXSdkDir());
  }
  getArkUIXSdkDir() {
    return this.property.getArkUIXSdkDir();
  }
}
