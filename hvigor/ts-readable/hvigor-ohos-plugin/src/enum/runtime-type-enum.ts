/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/enum/runtime-type-enum.js
 */
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
const _log = ohos_logger_js_1.OhosLogger.getLogger("RuntimeOS");
export var RuntimeTypeEnum;
!function (e) {
  e[e.UNDEFINED = 0] = "UNDEFINED", e[e.OpenHarmony = 1] = "OpenHarmony", e[e.HARMONY_OS = 2] = "HARMONY_OS";
}(RuntimeTypeEnum = RuntimeTypeEnum || (RuntimeTypeEnum = {})), function (e) {
  e.valueOf = function (n) {
    switch (n) {
      case undefined:
        return e.UNDEFINED;
      case "OpenHarmony":
        return e.OpenHarmony;
      case "HarmonyOS":
        return e.HARMONY_OS;
      default:
        throw _log.printErrorExit("INVALID_RUNTIME_OS_VALUE", [n]), new Error();
    }
  };
}(RuntimeTypeEnum = RuntimeTypeEnum || (RuntimeTypeEnum = {}));
