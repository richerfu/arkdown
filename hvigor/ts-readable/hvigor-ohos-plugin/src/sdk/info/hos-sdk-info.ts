/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/info/hos-sdk-info.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as sdkmanager_common_1 from "@ohos/sdkmanager-common";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as hmos_sdk_loader_js_1 from "../hmos-sdk-loader";
import * as hms_core_ets_component_js_1 from "../impl/hms-core-ets-component";
import * as hms_core_native_component_js_1 from "../impl/hms-core-native-component";
import * as sdk_java_component_js_1 from "../impl/sdk-java-component";
import * as sdk_toolchains_component_js_1 from "../impl/sdk-toolchains-component";
import * as sdk_util_js_1 from "../sdk-util";
import * as common_sdk_info_js_1 from "./common-sdk-info";
const _log = ohos_logger_js_1.OhosLogger.getLogger("hos-sdk-info");
export class HosSdkInfo extends common_sdk_info_js_1.CommonSdkInfo {
  constructor(o, e, s) {
    super(e, s), this.localHosLibComponents = new Map(), this.localLibComponents = new Map(), this.sdkDir = s, this.hosComponents = o, this.hmsCoreComponents = [sdkmanager_common_1.ComponentPath.TOOLCHAINS], this.hosComponents.forEach(o => {
      e.version > 9 && (o === sdkmanager_common_1.ComponentPath.ETS || o === sdkmanager_common_1.ComponentPath.NATIVE) && this.hmsCoreComponents.push(o);
    });
  }
  async setup() {
    const o = process.hrtime(),
      e = hmos_sdk_loader_js_1.HmosSdkLoader.getInstance();
    this.localLibComponents = await e.getHmosSdkComponents(this.sdkVersion, this.hosComponents), this.localHosLibComponents = await e.getHmsSdkComponents(this.sdkVersion, this.hmsCoreComponents), this.hosComponents.forEach(o => this.initComponent((0, sdk_util_js_1.contains)(o, this.localLibComponents))), this.hmsCoreComponents.forEach(o => this.hmsComponentInit((0, sdk_util_js_1.contains)(o, this.localHosLibComponents))), this.ohJavaComponent = new sdk_java_component_js_1.SdkJavaComponent((0, sdk_util_js_1.contains)(sdkmanager_common_1.ComponentPath.TOOLCHAINS, this.localLibComponents)), this.hasRollUpPluginInEtsLoader = fs_extra_1.existsSync(path_1.resolve(this.getEtsLoader(), sdk_const_js_1.ToolChainsConst.ROLL_UP_CONFIG_FILE)), this.hasRollUpPluginInJsLoader = fs_extra_1.existsSync(path_1.resolve(this.getJsLoader(), sdk_const_js_1.ToolChainsConst.ROLL_UP_CONFIG_FILE));
    const s = process.hrtime(o);
    _log.debug(`Sdk init in ${(0, hvigor_1.formatTime)(s)}`);
  }
  hmsComponentInit(o) {
    switch (o.getPath()) {
      case sdkmanager_common_1.ComponentPath.NATIVE:
        this.hmsCoreNativeComponent = new hms_core_native_component_js_1.HmsCoreNativeComponent(o);
        break;
      case sdkmanager_common_1.ComponentPath.ETS:
        this.hmsCoreEtsComponent = new hms_core_ets_component_js_1.HmsCoreEtsComponent(o);
        break;
      case sdkmanager_common_1.ComponentPath.TOOLCHAINS:
        this.hosJavaComponent = new sdk_java_component_js_1.SdkJavaComponent(o), this.hosToolchainsComponent = new sdk_toolchains_component_js_1.HosSdkToolchainsComponent(o);
    }
  }
  getHmsArkDir() {
    return this.hmsCoreEtsComponent.getBaseDir();
  }
  getHmsNativeDir() {
    return this.hmsCoreNativeComponent.getBaseDir();
  }
  getHmsToolchainFile() {
    return this.hmsCoreNativeComponent.getHmsToolchainFile();
  }
  getHmsBiShengToolchainFile() {
    return this.hmsCoreNativeComponent.getHmsBiShengToolchainFile();
  }
  getSdkBinxoTool() {
    return this.hmsCoreNativeComponent.getSdkBinxoTool();
  }
  getRestool() {
    var o, e;
    return null !== (e = null === (o = this.toolchainsComponent) || undefined === o ? undefined : o.getRestoolPath()) && undefined !== e ? e : "";
  }
  getHosResTool() {
    return this.hosJavaComponent.getRestoolPath();
  }
  getModuleSchema() {
    return this.hosToolchainsComponent.getHosModuleSchema();
  }
  getAppSchema() {
    return this.hosToolchainsComponent.getAppSchema();
  }
  getInsightIntentSchema() {
    return this.toolchainsComponent.getInsightIntentSchema();
  }
  getArkDataSchema() {
    return this.hosToolchainsComponent.getArkDataSchema();
  }
  getUtdSchema() {
    return this.toolchainsComponent.getUtdSchema();
  }
  getRichSchema() {
    return this.hosToolchainsComponent.getRichSchema();
  }
  getLiteSchema() {
    return this.hosToolchainsComponent.getLiteSchema();
  }
  getHapSignToolV2() {
    return this.hosToolchainsComponent.getHapSignToolV2();
  }
  getHosModuleSchema() {
    return this.hosToolchainsComponent.getHosModuleSchema();
  }
  getReleaseType() {
    return this.hosToolchainsComponent.getReleaseType();
  }
  getHosToolchainsComponentVersion() {
    return this.hosToolchainsComponent.getComponentVersion().getVersion();
  }
  getHosToolchainsDir() {
    return this.hosToolchainsComponent.getBaseDir();
  }
  getHosToolchainsLibDir() {
    return this.hosToolchainsComponent.getHosToolchainsLibDir();
  }
  getLibimageTranscoderShared() {
    return this.hosToolchainsComponent.getLibimageTranscoderShared();
  }
  getHosSignTool() {
    return this.hosToolchainsComponent.getHosSignTool();
  }
  getPacSchema() {
    return this.hosToolchainsComponent.getPacSchema();
  }
  getSystemThemeSchema() {
    return this.hosToolchainsComponent ? this.hosToolchainsComponent.getSystemThemeSchema() : "";
  }
}
