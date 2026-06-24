/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/strategy.js
 */
export function getStrategy(t, e) {
  var o;
  const r = null === (o = null == t ? undefined : t[0]) || undefined === o ? undefined : o.strategy;
  let n = null != e ? e : r;
  for (const e of t) if (null == e ? undefined : e.condition) {
    n = e.strategy;
    break;
  }
  return n;
}
