/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/entry/cache-entry.js
 */
export class CacheEntry {
  constructor(t, e) {
    this._key = t, this._content = e;
  }
  getKey() {
    return this._key;
  }
  getContent() {
    return this._content;
  }
}
