/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/use-new-ohm-url-plugin.js
 */
const NEW_OHM_URL_HASH = "newOhmUrlHash";
export function newOhmUrlPlugin() {
  return {
    name: "newOhmUrlPlugin",
    shouldInvalidCache: shouldInvalidCache
  };
}
function shouldInvalidCache() {
  return this.share?.projectConfig?.useNormalizedOHMUrl !== this.cache?.get("newOhmUrlHash");
}
