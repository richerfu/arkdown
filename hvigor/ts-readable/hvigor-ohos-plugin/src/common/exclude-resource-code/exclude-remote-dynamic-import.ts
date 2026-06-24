/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/exclude-resource-code/exclude-remote-dynamic-import.js
 */
export function collectFromBuildOption(e) {
  var o;
  return [...(null !== (o = null == e ? undefined : e.excludePackages) && undefined !== o ? o : [])];
}
export function isNeedToExcludeHar(e, o, l) {
  return !!e && e.has(l) && o.some(e => e.getDependencyName() === l);
}
