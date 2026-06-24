/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/enum/module-type-enum.js
 */
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
const _log = ohos_logger_js_1.OhosLogger.getLogger("ModuleType");
export var ModuleType;
!function (e) {
  e.Entry = "entry", e.Feature = "feature", e.Har = "har", e.Shared = "shared";
}(ModuleType = ModuleType || (ModuleType = {})), function (e) {
  e.valueOf = function (r) {
    switch (r) {
      case "entry":
        return e.Entry;
      case "feature":
        return e.Feature;
      case "har":
        return e.Har;
      case "shared":
        return e.Shared;
      default:
        return _log.printErrorExit("INVALID_MODULE_TYPE_VALUE", [r]), e.Entry;
    }
  };
}(ModuleType = ModuleType || (ModuleType = {}));
