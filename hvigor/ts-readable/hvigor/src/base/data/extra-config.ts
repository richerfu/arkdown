/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/data/extra-config.js
 */
let extraConfig = new Map();
export function setExtraConfig(t) {
  extraConfig = t;
}
export function getExtraConfig(t) {
  return extraConfig.get(t);
}
