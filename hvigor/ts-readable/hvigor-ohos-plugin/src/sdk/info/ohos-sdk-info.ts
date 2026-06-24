/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/info/ohos-sdk-info.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as sdkmanager_common_1 from "@ohos/sdkmanager-common";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as meta_util_js_1 from "../../utils/meta-util";
import * as ohos_sdk_loader_js_1 from "../ohos-sdk-loader";
import * as common_sdk_info_js_1 from "./common-sdk-info";
const _log = ohos_logger_js_1.OhosLogger.getLogger("ohos-sdk-info");
export class OhosSdkInfo extends common_sdk_info_js_1.CommonSdkInfo {
  constructor(o, s, e) {
    super(s, e), this.ohosLocalComponents = new Map(), this.isOhos = true, this.ohComponents = o, this.setOhosMetaCompileSdkVersion();
  }
  setOhosMetaCompileSdkVersion() {
    (0, meta_util_js_1.updateMetaCompileSdkVersion)(this.sdkVersion.version);
  }
  async setup() {
    const o = process.hrtime(),
      s = [],
      e = new sdkmanager_common_1.ApiVersion(this.sdkVersion.fullVersion);
    this.ohComponents.forEach(o => s.push(new sdkmanager_common_1.PathAndApiVersion(o, e))), this.ohosLocalComponents = await ohos_sdk_loader_js_1.OhosSdkLoader.getInstance().getOhosSdkComponents(this.sdkVersion, this.ohComponents), s.forEach(o => this.initComponent(this.contains(o, this.ohosLocalComponents))), this.hasRollUpPluginInJsLoader = fs_extra_1.existsSync(path_1.resolve(this.getJsLoader(), sdk_const_js_1.ToolChainsConst.ROLL_UP_CONFIG_FILE)), this.hasRollUpPluginInEtsLoader = fs_extra_1.existsSync(path_1.resolve(this.getEtsLoader(), sdk_const_js_1.ToolChainsConst.ROLL_UP_CONFIG_FILE));
    const t = process.hrtime(o);
    _log.debug(`Sdk init in ${(0, hvigor_1.formatTime)(t)}`);
  }
  contains(o, s) {
    for (const [e, t] of s) if (e.equals(o)) return t;
  }
  getLibimageTranscoderShared() {
    return "";
  }
  getHosToolchainsLibDir() {
    return "";
  }
  getSystemThemeSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getSystemThemeSchema() : "";
  }
  getArkDataSchema() {
    return this.toolchainsComponent.getArkDataSchema();
  }
}
