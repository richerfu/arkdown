/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/entry/tree-cache-entry.js
 */
import * as cache_entry_js_1 from "./cache-entry";
export class TreeCacheEntry extends cache_entry_js_1.CacheEntry {
  constructor(e, t) {
    super(e, t), this._subEntries = new Map();
  }
  getSubEntry(e) {
    return this._subEntries.get(e);
  }
  setSubEntry(e, t) {
    this._subEntries.set(e, t);
  }
  getSunEntrySize() {
    return this._subEntries.size;
  }
  remove(e) {
    this._subEntries.delete(e);
  }
}
