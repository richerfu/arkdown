/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/replacer.js
 */
export function replacer(e, r) {
  if (r instanceof Map) {
    const e = Object.create(null);
    return r.forEach((r, t) => {
      e[t] = r;
    }), e;
  }
  if (r instanceof Set) {
    const e = [];
    return r.forEach(r => {
      e.push(r);
    }), e;
  }
  return r;
}
