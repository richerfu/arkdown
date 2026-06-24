/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/enum/code-type-enum.js
 */
import * as sdkmanager_common_1 from "@ohos/sdkmanager-common";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
const _log = ohos_logger_js_1.OhosLogger.getLogger("CodeType");
export var CodeType, SdkTypeEnum;
!function (e) {
  e.JS = "js", e.ETS = "ets", e.CPP = "cpp";
}(CodeType = CodeType || (CodeType = {})), function (e) {
  e.HOS = "HOS", e.OHOS = "OHOS", e.ARKUI_X = "ARKUI-X";
}(SdkTypeEnum = SdkTypeEnum || (SdkTypeEnum = {})), function (e) {
  e.getSDKComponentName = function (o) {
    switch (o) {
      case e.CPP.toLowerCase():
        return sdkmanager_common_1.ComponentPath.NATIVE;
      case e.ETS.toLowerCase():
        return sdkmanager_common_1.ComponentPath.ETS;
      case e.JS.toLowerCase():
        return sdkmanager_common_1.ComponentPath.JS;
      default:
        return _log._buildError(`This type ${o} of source code is not supported!`)._solution('Value should be one of: "js", "ets", "cpp"')._printErrorAndExit(), "";
    }
  };
}(CodeType = CodeType || (CodeType = {}));
