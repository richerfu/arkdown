/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/util/cache-util.js
 */
import * as class_type_const_js_1 from "../../../common/options/class-type-const";
import * as class_identify_util_js_1 from "../../../util/class-identify-util";
export function getTaskSnapCacheEntryUniqueKey(e, t) {
  const s = t.getProject().getName();
  return (0, class_identify_util_js_1.instanceOf)(t, class_type_const_js_1.ClassTypeConst.HVIGOR_MODULE) ? `:${s}:${t.getName()}:${e}` : `:${s}:${e}`;
}
