/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/meta-util.js
 */
import * as hvigor_1 from "@ohos/hvigor";
function updateToolChainsVersionWhenChanged(o) {
  const e = "toolChainsVersion",
    r = o.getToolchainsComponentVersion();
  return hvigor_1.hvigorCore.getMetaProperty(e) !== r && (hvigor_1.hvigorCore.updateMetaProperty(e, r), true);
}
export function hvigorOrToolChainsChanged(o) {
  const e = updateToolChainsVersionWhenChanged(o);
  return hvigor_1.hvigorCore.versionChanged() || e;
}
export function updateMetaCompileSdkVersion(o) {
  hvigor_1.hvigorCore.updateMetaProperty("compileSdkVersion", o);
}
