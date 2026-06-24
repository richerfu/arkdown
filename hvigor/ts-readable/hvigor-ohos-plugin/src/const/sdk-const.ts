/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/const/sdk-const.js
 */
import * as sdkmanager_common_1 from "@ohos/sdkmanager-common";
export class ToolChainsConst {}
export const SDK_COMPONENTS = [sdkmanager_common_1.ComponentPath.TOOLCHAINS, sdkmanager_common_1.ComponentPath.ETS, sdkmanager_common_1.ComponentPath.JS, sdkmanager_common_1.ComponentPath.NATIVE, sdkmanager_common_1.ComponentPath.PREVIEWER];
ToolChainsConst.SIGN_TOOL = "hapsigntoolv2.jar", ToolChainsConst.NEW_SIGN_TOOL = "hap-sign-tool.jar", ToolChainsConst.SDK_SIGN_TOOL_FULL = "sdk-sign-tool-full.jar", ToolChainsConst.APP_PACKING_TOOL_JAR = "app_packing_tool.jar", ToolChainsConst.APP_UN_PACKING_TOOL_JAR = "app_unpacking_tool.jar", ToolChainsConst.HMOS_APP_PACKING_TOOL_JAR = "hmos_app_packing_tool.jar", ToolChainsConst.HAP_TO_BIN_TOOL_JAR = "haptobin_tool.jar", ToolChainsConst.ROLL_UP_CONFIG_FILE = "rollup.config.js";
export class ApiVersion {}
export var AppPackingToolMode;
ApiVersion.API_VERSION_20 = 20, ApiVersion.API_VERSION_16 = 16, ApiVersion.API_VERSION_14 = 14, ApiVersion.API_VERSION_12 = 12, ApiVersion.API_VERSION_11 = 11, ApiVersion.API_VERSION_10 = 10, ApiVersion.API_VERSION_9 = 9, ApiVersion.API_VERSION_8 = 8, ApiVersion.DISTRO_FILTER_MIN_API_VERSION = 3, ApiVersion.DISTRO_FILTER_MAX_API_VERSION = 30, function (o) {
  o.APP = "app", o.FastApp = "fastApp", o.GeneralNormalize = "generalNormalize";
}(AppPackingToolMode = AppPackingToolMode || (AppPackingToolMode = {}));
