/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/class-identify-util.js
 */
export function instanceOf(e, n) {
  return undefined !== e && e.classKind === n;
}
