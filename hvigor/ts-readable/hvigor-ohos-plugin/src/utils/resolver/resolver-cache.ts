/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/resolver/resolver-cache.js
 */
export class ResolverCache {
  static has(e) {
    return ResolverCache.cacheMap.has(e);
  }
  static get(e) {
    return ResolverCache.cacheMap.get(e);
  }
  static set(e, a) {
    return ResolverCache.cacheMap.set(e, a);
  }
  static clear() {
    return ResolverCache.cacheMap.clear();
  }
}
ResolverCache.cacheMap = new Map();
