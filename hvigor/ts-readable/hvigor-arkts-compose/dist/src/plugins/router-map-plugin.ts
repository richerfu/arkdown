/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/router-map-plugin.js
 */
const ROUTER_MAP_HASH = "routerMapHash";
export function routerMapPlugin() {
  return {
    name: "routerMapPlugin",
    shouldInvalidCache: shouldInvalidCache
  };
}
function shouldInvalidCache(e) {
  return this.share?.projectConfig?.routerMap?.get(e.id) !== this.cache?.get("routerMapHash")?.get(e.id);
}
