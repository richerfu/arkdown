/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/service/default-cache-service.js
 */
export class DefaultCacheService {
  constructor() {
    this.cacheEntries = new Map();
  }
  clone(e) {
    e.keys().forEach(t => {
      this.set(t, e.get(t));
    });
  }
  initialize() {}
  close() {
    this.cacheEntries.clear();
  }
  get(e) {
    var t;
    return null === (t = this.cacheEntries.get(e)) || undefined === t ? undefined : t.getContent();
  }
  remove(e) {
    this.cacheEntries.delete(e);
  }
  size() {
    return this.cacheEntries.size;
  }
  keys() {
    return [...this.cacheEntries.keys()];
  }
  getCacheEntryContentMap() {
    const e = new Map();
    return this.cacheEntries.forEach((t, c) => {
      e.set(c, t.getContent());
    }), e;
  }
}
