/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/object-util.js
 */
export const freezeDeep = e => {
  Object.freeze(e);
  for (const r of Object.values(e)) "object" != typeof r || Object.isFrozen(r) || (0, freezeDeep)(r);
  return e;
};
