/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/sdk-previewer-component.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as json_util_js_1 from "../../utils/json-util";
import * as sdk_component_js_1 from "../superior/sdk-component";
export class SdkPreviewerComponent extends sdk_component_js_1.SdkComponent {
  constructor(e) {
    super(e), this.logger = hvigor_1.HvigorLogger.getLogger(SdkPreviewerComponent.name), this.sdkDefinePermissions = new Map(), this.allowSdkPermission = false, this.setPermissionData();
  }
  getCommonBin() {
    return path_1.resolve(this.getBaseDir(), SdkPreviewerComponent.COMMON, SdkPreviewerComponent.BIN);
  }
  setPermissionData() {
    var e, s;
    this.logger.debug("Start recording SDK configuration permission data.");
    const t = path_1.resolve(this.getBaseDir(), SdkPreviewerComponent.COMMON, SdkPreviewerComponent.RESOURCES_MODULE);
    if (!fs_1.existsSync(t)) return void this.logger.debug(`The file does not exist. At ${t}`);
    const i = null === (s = null === (e = json_util_js_1.JsonUtil.getJson5Obj(t)) || undefined === e ? undefined : e.module) || undefined === s ? undefined : s.definePermissions;
    i ? (i.forEach(e => {
      const s = e.grantMode,
        t = this.sdkDefinePermissions.get(s) || [];
      this.sdkDefinePermissions.set(s, [...t, e.name]);
    }), this.allowSdkPermission = i.length > 0) : this.logger.warn(`Failed to obtain the valid permission list. Download the SDK again.\nat ${t} `);
  }
  getSdkPermissionNames() {
    return Array.from(this.sdkDefinePermissions.values()).reduce((e, s) => [...e, ...s], []);
  }
  getSdkDefineUserGrant() {
    return this.sdkDefinePermissions.get("user_grant") || [];
  }
  getSdkManualSettings() {
    return this.sdkDefinePermissions.get("manual_settings") || [];
  }
}
SdkPreviewerComponent.COMMON = "common", SdkPreviewerComponent.BIN = "bin", SdkPreviewerComponent.RESOURCES_MODULE = "resources/module.json";
