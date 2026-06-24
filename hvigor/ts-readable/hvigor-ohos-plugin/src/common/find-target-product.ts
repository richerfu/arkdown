/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/find-target-product.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as array_util_js_1 from "../utils/array-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
const _log = ohos_logger_js_1.OhosLogger.getLogger("find-target-product");
export function findTargetProduct(r) {
  const o = hvigor_1.hvigorCore.getExtraConfig().get("product"),
    t = o || "default",
    e = (0, array_util_js_1.getElementFromArr)(r.getProfileOpt().app.products, t);
  return e || _log.printErrorExit("PRODUCT_NAME_NO_FOUND", [t, r.getName(), r.getProfilePath()]), e;
}
