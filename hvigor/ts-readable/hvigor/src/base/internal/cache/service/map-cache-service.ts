/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/service/map-cache-service.js
 */
export class MapCacheService {
  constructor() {
    this.cacheEntryMap = new Map();
  }
  initialize() {}
  close() {
    this.cacheEntryMap.clear();
  }
  get(e) {
    return this.cacheEntryMap.get(e);
  }
  remove(e) {
    this.cacheEntryMap.delete(e);
  }
  size() {
    return this.cacheEntryMap.size;
  }
}
