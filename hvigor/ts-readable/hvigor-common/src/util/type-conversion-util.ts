/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/type-conversion-util.js
 */
export function strMapToObj(t) {
  const o = Object.create(null);
  for (const [e, r] of t) r instanceof Map ? o[e] = strMapToObj(r) : o[e] = r;
  return o;
}
