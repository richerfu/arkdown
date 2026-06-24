/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/sdk-toolchains-component.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as common_const_js_1 from "../../const/common-const";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as sdk_version_enum_js_1 from "../../version/sdk-version-enum";
import * as sdk_component_js_1 from "../superior/sdk-component";
export class SdkToolchainsComponent extends sdk_component_js_1.SdkComponent {
  constructor(o) {
    super(o);
  }
  getRestoolPath() {
    const o = (0, hvigor_1.isWindows)() ? "restool.exe" : "restool";
    return path_1.resolve(this.getBaseDir(), o);
  }
  getSysCapToolPath() {
    const o = (0, hvigor_1.isWindows)() ? "syscap_tool.exe" : "syscap_tool";
    return path_1.resolve(this.getBaseDir(), o);
  }
  getVerifySignConfigToolPath() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.LIB, "hap-sign-tool.jar");
  }
  getRichSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.CONFIG_CHECK, SdkToolchainsComponent.RICH_SCHEMA);
  }
  getLiteSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.CONFIG_CHECK, SdkToolchainsComponent.LITE_SCHEMA);
  }
  getPackageToolPath() {
    const o = this.getComponentVersion().isHigherThan(sdk_version_enum_js_1.SdkVersionEnum.NEW_PACK_TOOL) ? sdk_const_js_1.ToolChainsConst.APP_PACKING_TOOL_JAR : sdk_const_js_1.ToolChainsConst.HMOS_APP_PACKING_TOOL_JAR;
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.LIB, o);
  }
  getUnPackageToolPath() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.LIB, sdk_const_js_1.ToolChainsConst.APP_UN_PACKING_TOOL_JAR);
  }
  getSignDir() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.LIB);
  }
  getModuleSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.MODULE_JSON);
  }
  getAppSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.APP_SCHEMA_JSON);
  }
  getAppConfigurationSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.CONFIGURATION_JSON);
  }
  getInsightIntentSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.INSIGHT_INTENT_SCHEMA_JSON);
  }
  getArkDataSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.ARK_DATA_SCHEMA_JSON);
  }
  getUtdSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.UTD_SCHEMA_JSON);
  }
  getFormSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.FORM_SCHEMA_JSON);
  }
  getPageSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.PAGE_SCHEMA_JSON);
  }
  getRouterMapSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.ROUTER_MAP_SCHEMA_JSON);
  }
  getShareConfigSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.SHARE_CONFIG_SCHEMA_JSON);
  }
  getShortcutsJsonSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.SHORTCUTS_SCHEMA_JSON);
  }
  getAppStartupSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.APP_STARTUP_SCHEMA_JSON);
  }
  getHspStartupSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.HSP_STARTUP_SCHEMA_JSON);
  }
  getHarStartupSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.HAR_STARTUP_SCHEMA_JSON);
  }
  getStartWindowSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.START_WINDOW_SCHEMA_JSON);
  }
  getSysCapSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.SYS_CAP_CHECK, common_const_js_1.CommonConst.SYS_CAP_SCHEMA_JSON);
  }
  getPacSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.PAC_CHECK, common_const_js_1.CommonConst.PAC_SCHEMA_JSON);
  }
  getResIdDefinedSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.RES_CHECK, common_const_js_1.CommonConst.ID_DEFINED_SCHEMA_JSON);
  }
  getSystemThemeSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.SYSTEM_THEME_JSON);
  }
}
SdkToolchainsComponent.MODULE_CHECK = "modulecheck", SdkToolchainsComponent.CONFIG_CHECK = "configcheck", SdkToolchainsComponent.SYS_CAP_CHECK = "syscapcheck", SdkToolchainsComponent.PAC_CHECK = "paccheck", SdkToolchainsComponent.RES_CHECK = "rescheck", SdkToolchainsComponent.LIB = "lib", SdkToolchainsComponent.LIBIMAGE_TRANSCODER_SHARED_DLL = "libimage_transcoder_shared.dll", SdkToolchainsComponent.LIBIMAGE_TRANSCODER_SHARED_DYLIB = "libimage_transcoder_shared.dylib", SdkToolchainsComponent.LIBIMAGE_TRANSCODER_SHARED_SO = "libimage_transcoder_shared.so", SdkToolchainsComponent.RICH_SCHEMA = "configSchema_rich.json", SdkToolchainsComponent.LITE_SCHEMA = "configSchema_lite.json";
export class HosSdkToolchainsComponent extends SdkToolchainsComponent {
  constructor(o) {
    super(o), this.component = o;
  }
  getModuleSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.MODULE_JSON);
  }
  getAppSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.APP_SCHEMA_JSON);
  }
  getRichSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.CONFIG_CHECK, SdkToolchainsComponent.RICH_SCHEMA);
  }
  getLiteSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.CONFIG_CHECK, SdkToolchainsComponent.LITE_SCHEMA);
  }
  getHapSignToolV2() {
    return path_1.resolve(this.getBaseDir(), HosSdkToolchainsComponent.LIB, sdk_const_js_1.ToolChainsConst.SIGN_TOOL);
  }
  getHosModuleSchema() {
    return path_1.resolve(this.getBaseDir(), SdkToolchainsComponent.MODULE_CHECK, common_const_js_1.CommonConst.MODULE_JSON);
  }
  getReleaseType() {
    return this.component.getReleaseType();
  }
  getHosSignTool() {
    return path_1.resolve(this.getBaseDir(), HosSdkToolchainsComponent.LIB, sdk_const_js_1.ToolChainsConst.SDK_SIGN_TOOL_FULL);
  }
  getHosToolchainsLibDir() {
    return path_1.resolve(this.getBaseDir(), HosSdkToolchainsComponent.LIB);
  }
  getLibimageTranscoderShared() {
    const o = (0, hvigor_1.isWindows)() ? HosSdkToolchainsComponent.LIBIMAGE_TRANSCODER_SHARED_DLL : (0, hvigor_1.isLinux)() ? HosSdkToolchainsComponent.LIBIMAGE_TRANSCODER_SHARED_SO : HosSdkToolchainsComponent.LIBIMAGE_TRANSCODER_SHARED_DYLIB;
    return path_1.resolve(this.getHosToolchainsLibDir(), o);
  }
}
