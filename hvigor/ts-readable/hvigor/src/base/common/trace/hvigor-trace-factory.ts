/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/trace/hvigor-trace-factory.js
 */
import * as hvigor_trace_js_1 from "./hvigor-trace";
export function TrackAPI(r, e, t) {
  const c = t.value;
  return t.value = function (...r) {
    return hvigor_trace_js_1.hvigorTrace.insertUsedApi(e), c.apply(this, r);
  }, t;
}
