/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/info/common-sdk-info.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as sdkmanager_common_1 from "@ohos/sdkmanager-common";
import fs_extra_1 from "fs-extra";
import * as inject_const_js_1 from "../../const/inject-const";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as sdk_version_enum_js_1 from "../../version/sdk-version-enum";
import * as default_sdk_js_1 from "../default-sdk";
import * as sdk_ets_component_js_1 from "../impl/sdk-ets-component";
import * as sdk_js_component_js_1 from "../impl/sdk-js-component";
import * as sdk_native_component_js_1 from "../impl/sdk-native-component";
import * as sdk_previewer_component_js_1 from "../impl/sdk-previewer-component";
import * as sdk_toolchains_component_js_1 from "../impl/sdk-toolchains-component";
export class CommonSdkInfo extends default_sdk_js_1.DefaultSdk {
  constructor(t, e) {
    super(), this._log = ohos_logger_js_1.OhosLogger.getLogger(CommonSdkInfo.name), this.sdkVersion = t, this.sdkDir = e;
  }
  getToolchainsComponentVersion() {
    var t;
    return null === (t = this.toolchainsComponent) || undefined === t ? undefined : t.getComponentVersion().getVersion();
  }
  getModuleSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getModuleSchema() : "";
  }
  getAppSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getAppSchema() : "";
  }
  getAppConfigurationSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getAppConfigurationSchema() : "";
  }
  getStartWindowSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getStartWindowSchema() : "";
  }
  getSystemThemeSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getSystemThemeSchema() : "";
  }
  getInsightIntentSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getInsightIntentSchema() : "";
  }
  getUtdSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getUtdSchema() : "";
  }
  getFormSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getFormSchema() : "";
  }
  getPageSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getPageSchema() : "";
  }
  getRouterMapSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getRouterMapSchema() : "";
  }
  getShareConfigSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getShareConfigSchema() : "";
  }
  getShortcutsJsonSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getShortcutsJsonSchema() : "";
  }
  getAppStartupSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getAppStartupSchema() : "";
  }
  getHspStartupSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getHspStartupSchema() : "";
  }
  getHarStartupSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getHarStartupSchema() : "";
  }
  getSysCapSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getSysCapSchema() : "";
  }
  getPacSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getPacSchema() : "";
  }
  getResIdDefinedSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getResIdDefinedSchema() : "";
  }
  getSdkJsDir() {
    return this.jsComponent ? this.jsComponent.getBaseDir() : "";
  }
  getSdkEtsDir() {
    return this.etsComponent ? this.etsComponent.getBaseDir() : "";
  }
  getSdkVersion() {
    return this.sdkVersion.version;
  }
  getSdkApi() {
    return this.sdkVersion.api;
  }
  getHapTobin() {
    return this.jsComponent.getHapToBinFilePath();
  }
  getReleaseType() {
    return this.toolchainsComponent ? this.toolchainsComponent.getReleaseType() : "Release";
  }
  getSdkNativeDir() {
    return this.nativeComponent.getBaseDir();
  }
  getCmakeTool() {
    return this.nativeComponent.getCmakeTool();
  }
  getNativeNinjaTool() {
    return this.nativeComponent.getNinjaTool();
  }
  getNativeToolchain() {
    return this.nativeComponent.getNativeToolchain();
  }
  getNdkVersion() {
    return this.nativeComponent.getComponentVersion();
  }
  getSdkToolchainsDir() {
    return this.toolchainsComponent ? this.toolchainsComponent.getBaseDir() : "";
  }
  getSdkLlvmStrip() {
    return this.nativeComponent.getLlvmStrip();
  }
  getSdkLlvmReadElf() {
    return this.nativeComponent.getLlvmReadElf();
  }
  getRestool() {
    return this.toolchainsComponent ? this.toolchainsComponent.getRestoolPath() : "";
  }
  getSysCapTool() {
    return this.toolchainsComponent ? this.toolchainsComponent.getSysCapToolPath() : "";
  }
  getSysCapFileInEts() {
    return this.etsComponent ? this.etsComponent.getSysCapFilePath() : "";
  }
  getSysCapFileInJs() {
    return this.jsComponent ? this.jsComponent.getSysCapFilePath() : "";
  }
  getVerifySignConfigTool() {
    return this.toolchainsComponent ? this.toolchainsComponent.getVerifySignConfigToolPath() : "";
  }
  getJsArkVersion(t, e) {
    if (!this.jsComponent) return "";
    if (this.findNoAbcTool(this.jsComponent.getJsLoaderPath())) return this._log.warn("Ark tools not found. Cannot get ark version."), "";
    const o = this.jsComponent.getCompatibleArkVersion(t, e);
    return o && "" !== o ? o : this.jsComponent.getArkVersion();
  }
  getRichSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getRichSchema() : "";
  }
  getLiteSchema() {
    return this.toolchainsComponent ? this.toolchainsComponent.getLiteSchema() : "";
  }
  getEtsArkVersion(t, e) {
    if (!this.etsComponent) return "";
    if (this.findNoAbcTool(this.etsComponent.getEtsLoaderPath())) return this._log.warn("Ark tools not found. Cannot get ark version."), "";
    const o = this.etsComponent.getCompatibleArkVersion(t, e);
    return o && "" !== o ? o : this.etsComponent.getArkVersion();
  }
  getEtsComponentVersion() {
    return this.etsComponent ? this.etsComponent.getComponentVersion().getVersion() : "";
  }
  getEtsComponentReleaseType() {
    return this.etsComponent ? this.etsComponent.getReleaseType() : "Release";
  }
  getArkVersion(t, e) {
    const o = this.getEtsArkVersion(t, e);
    return o && "" !== o ? o : this.getJsArkVersion(t, e);
  }
  getPackageTool() {
    return this.toolchainsComponent ? this.toolchainsComponent.getPackageToolPath() : "";
  }
  getUnPackageTool() {
    return this.toolchainsComponent ? this.toolchainsComponent.getUnPackageToolPath() : "";
  }
  getBasePath() {
    var t, e;
    return null !== (e = null === (t = this.toolchainsComponent) || undefined === t ? undefined : t.getBaseDir()) && undefined !== e ? e : "";
  }
  getPreviewerCommonBinDir() {
    var t, e;
    return null !== (e = null === (t = this.previewerComponent) || undefined === t ? undefined : t.getCommonBin()) && undefined !== e ? e : "";
  }
  getPreviewerPermissionNames() {
    var t, e;
    return null !== (e = null === (t = this.previewerComponent) || undefined === t ? undefined : t.getSdkPermissionNames()) && undefined !== e ? e : [];
  }
  getPreviewerUserGrant() {
    var t, e;
    return null !== (e = null === (t = this.previewerComponent) || undefined === t ? undefined : t.getSdkDefineUserGrant()) && undefined !== e ? e : [];
  }
  getPreviewerManualSettings() {
    var t, e;
    return null !== (e = null === (t = this.previewerComponent) || undefined === t ? undefined : t.getSdkManualSettings()) && undefined !== e ? e : [];
  }
  allowSdkPermission() {
    var t, e;
    return null !== (e = null === (t = this.previewerComponent) || undefined === t ? undefined : t.allowSdkPermission) && undefined !== e && e;
  }
  getSignDir() {
    return this.toolchainsComponent ? this.toolchainsComponent.getSignDir() : "";
  }
  getJsLoader() {
    return this.jsComponent ? this.jsComponent.getJsLoaderPath() : "";
  }
  getEtsLoader() {
    return this.etsComponent ? this.etsComponent.getEtsLoaderPath() : "";
  }
  requireUISyntax() {
    return undefined !== this.jsComponent && undefined !== this.etsComponent ? this.jsComponent.requireUISyntax() || this.etsComponent.requireUISyntax() : undefined !== this.etsComponent ? this.etsComponent.requireUISyntax() : undefined !== this.jsComponent && this.jsComponent.requireUISyntax();
  }
  isPreviewCompileResourceIncrement(t) {
    var e;
    const o = hvigor_1.hvigorCore.getExtraConfig().get(inject_const_js_1.InjectConst.PREVIEWER_COMPILE_RES_INC),
      n = this.sdkVersion.version >= 9;
    return "true" === o && n && !!(null === (e = this.toolchainsComponent) || undefined === e ? undefined : e.supportCompileResourceIncrement(t));
  }
  supportOhpmProject(t) {
    var e;
    return !!(null === (e = this.toolchainsComponent) || undefined === e ? undefined : e.supportOhpmProject(t));
  }
  supportHapSignToolApiCall(t) {
    var e, o;
    const n = null === (e = this.toolchainsComponent) || undefined === e ? undefined : e.getComponentApiVersion(),
      s = null === (o = this.toolchainsComponent) || undefined === o ? undefined : o.getComponentVersion();
    return !(!n || !s) && (n > 12 || (12 === n ? s.isHigherThan(sdk_version_enum_js_1.SdkVersionEnum.SUPPORT_HAP_SIGN_TOOL_API_CALL_API12) : 11 === n && (t ? s.isHigherThan(sdk_version_enum_js_1.SdkVersionEnum.HOS_SUPPORT_HAP_SIGN_TOOL_API_CALL_API11) : s.isHigherThan(sdk_version_enum_js_1.SdkVersionEnum.OHOS_SUPPORT_HAP_SIGN_TOOL_API_CALL_API11))));
  }
  initComponent(t) {
    switch (t.getPath()) {
      case sdkmanager_common_1.ComponentPath.JS:
        this.jsComponent = new sdk_js_component_js_1.SdkJsComponent(t);
        break;
      case sdkmanager_common_1.ComponentPath.ETS:
        this.etsComponent = new sdk_ets_component_js_1.SdkEtsComponent(t);
        break;
      case sdkmanager_common_1.ComponentPath.TOOLCHAINS:
        this.toolchainsComponent = new sdk_toolchains_component_js_1.SdkToolchainsComponent(t);
        break;
      case sdkmanager_common_1.ComponentPath.NATIVE:
        this.nativeComponent = new sdk_native_component_js_1.SdkNativeComponent(t);
        break;
      case sdkmanager_common_1.ComponentPath.PREVIEWER:
        this.previewerComponent = new sdk_previewer_component_js_1.SdkPreviewerComponent(t);
    }
  }
  findNoAbcTool(t) {
    const e = path_1.resolve(t, "bin", "ark", "ts2abc.js");
    return !e || !fs_extra_1.existsSync(e);
  }
}
