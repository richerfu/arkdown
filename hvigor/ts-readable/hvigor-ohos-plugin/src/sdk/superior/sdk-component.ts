/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/superior/sdk-component.js
 */
import * as sdk_version_js_1 from "../../version/sdk-version";
import * as sdk_version_enum_js_1 from "../../version/sdk-version-enum";
export class SdkComponent {
  constructor(e) {
    this._baseDir = e.getLocation(), this._componentApiVersion = e.getFullApiVersion().getMajor(), this._componentVersion = new sdk_version_js_1.SdkVersion(e.getVersion()), this._releaseType = e.getReleaseType();
  }
  getBaseDir() {
    return this._baseDir;
  }
  getComponentApiVersion() {
    return this._componentApiVersion;
  }
  getComponentVersion() {
    return this._componentVersion;
  }
  getReleaseType() {
    return this._releaseType;
  }
  requireUISyntax() {
    return !this._componentVersion.isHigherThan(sdk_version_enum_js_1.SdkVersionEnum.REQUIRE_UI_SYNTAX) && !this._componentVersion.equals(sdk_version_enum_js_1.SdkVersionEnum.SUPPORT_UI_SYNTAX);
  }
  supportCompileResourceIncrement(e) {
    return e ? this._componentVersion.isHigherThan(sdk_version_enum_js_1.SdkVersionEnum.HOS_SUPPORT_COMPILE_RESOURCE_INCREMENT) || this._componentVersion.equals(sdk_version_enum_js_1.SdkVersionEnum.HOS_SUPPORT_COMPILE_RESOURCE_INCREMENT) : this._componentVersion.isHigherThan(sdk_version_enum_js_1.SdkVersionEnum.OHOS_SUPPORT_COMPILE_RESOURCE_INCREMENT) || this._componentVersion.equals(sdk_version_enum_js_1.SdkVersionEnum.OHOS_SUPPORT_COMPILE_RESOURCE_INCREMENT);
  }
  supportOhpmProject(e) {
    return e ? this._componentVersion.isHigherThan(sdk_version_enum_js_1.SdkVersionEnum.HOS_SUPPORT_OHPM_PROJECT) || this._componentVersion.equals(sdk_version_enum_js_1.SdkVersionEnum.HOS_SUPPORT_OHPM_PROJECT) : this._componentVersion.isHigherThan(sdk_version_enum_js_1.SdkVersionEnum.OHOS_SUPPORT_OHPM_PROJECT) || this._componentVersion.equals(sdk_version_enum_js_1.SdkVersionEnum.OHOS_SUPPORT_OHPM_PROJECT);
  }
}
