/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/default-sdk.js
 */
import * as sdk_version_enum_js_1 from "../version/sdk-version-enum";
import * as sdk_version_js_1 from "../version/sdk-version";
const NO_IMPL_EXCEPTION = "No impl exception";
export class DefaultSdk {
  constructor() {
    this.hasRollUpPluginInEtsLoader = true, this.hasRollUpPluginInJsLoader = true, this.isOhos = false, this.sdkDir = "";
  }
  async setup() {}
  checkAotFixedSdkVersion() {
    const r = sdk_version_enum_js_1.SdkVersionEnum.FIXED_AOT_OHOS,
      e = this.getEtsComponentVersion();
    if ("" === e) return true;
    return new sdk_version_js_1.SdkVersion(e).isHigherThan(r);
  }
  getAppSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getInsightIntentSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getArkDataSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getUtdSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getArkUIXSdkDir() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getArkVersion(r, e) {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getCmakeTool() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getEtsComponentReleaseType() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getEtsComponentVersion() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getHosToolchainsDir() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getEtsLoader() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getFormSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getHapSignToolV2() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getHapTobin() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getHmsArkDir() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getHmsNativeDir() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getHmsToolchainFile() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getHmsBiShengToolchainFile() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getHosModuleSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getHosResTool() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getJsLoader() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getLiteSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getModuleSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getNativeNinjaTool() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getNativeToolchain() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getPackageTool() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getUnPackageTool() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getPageSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getStartWindowSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getSystemThemeSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getRouterMapSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getShareConfigSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getShortcutsJsonSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getAppStartupSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getHspStartupSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getHarStartupSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getSysCapSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getPacSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getResIdDefinedSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getReleaseType() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getRestool() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getRichSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getSdkEtsDir() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getSdkJsDir() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getSdkNativeDir() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getSdkToolchainsDir() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getSdkBinxoTool() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getSdkLlvmStrip() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getSdkLlvmReadElf() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getSdkVersion() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getHosToolchainsLibDir() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getLibimageTranscoderShared() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getPreviewerCommonBinDir() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getPreviewerPermissionNames() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getPreviewerUserGrant() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getPreviewerManualSettings() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  allowSdkPermission() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getSignDir() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getHosSignTool() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getSysCapFileInEts() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getSysCapFileInJs() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getSysCapTool() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getToolchainsComponentVersion() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getHosToolchainsComponentVersion() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getVerifySignConfigTool() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  isPreviewCompileResourceIncrement(r) {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  requireUISyntax() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  supportOhpmProject(r) {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  supportHapSignToolApiCall(r) {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getSdkApi() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getBasePath() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
  getAppConfigurationSchema() {
    throw new Error(NO_IMPL_EXCEPTION);
  }
}
