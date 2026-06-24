/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/info/arkuix-sdk-info.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as sdkmanager_common_1 from "@ohos/sdkmanager-common";
import * as sdk_arkuix_component_js_1 from "../impl/sdk-arkuix-component";
import * as default_progress_js_1 from "../lib/default-progress";
import * as sdk_util_js_1 from "../sdk-util";
import * as common_sdk_info_js_1 from "./common-sdk-info";
export class ArkUIXSdkInfo extends common_sdk_info_js_1.CommonSdkInfo {
  constructor(o, s, n) {
    super(s, n), this.localLibComponents = new Map(), this.arkUIXComponents = o, this.projectArkUIXSdkPath = n;
    const e = sdkmanager_common_1.SdkInfoConfig.create(n).setSdkProxy(sdk_util_js_1.proxyFun);
    this.arkUIXSdkInfoHandler = new sdkmanager_common_1.ArkUIXSdkInfoHandler(e, new default_progress_js_1.DefaultProgress());
  }
  async setup() {
    const o = process.hrtime(),
      s = [],
      n = new sdkmanager_common_1.ApiVersion(this.sdkVersion.fullVersion);
    this.arkUIXComponents.forEach(o => s.push(new sdkmanager_common_1.PathAndApiVersion(o, n)));
    try {
      this.localLibComponents = await this.arkUIXSdkInfoHandler.getOrDownload(s);
    } catch (o) {
      (0, sdk_util_js_1.handleSdkException)(o);
    }
    s.forEach(o => this.initComponent(this.contains(o, this.localLibComponents)));
    const e = process.hrtime(o);
    this._log.debug(`Sdk init in ${(0, hvigor_1.formatTime)(e)}`);
  }
  contains(o, s) {
    for (const [n, e] of s) if (n.equals(o)) return e;
  }
  initComponent(o) {
    this.sdkArkUIXComponent = new sdk_arkuix_component_js_1.SdkArkUIXComponent(o);
  }
  getArkUIXRootPath() {
    return this.sdkArkUIXComponent.getBaseDir();
  }
  getProjectArkUIXSdkPath() {
    return this.projectArkUIXSdkPath;
  }
}
