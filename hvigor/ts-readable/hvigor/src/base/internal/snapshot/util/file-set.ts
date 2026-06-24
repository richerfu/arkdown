/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/util/file-set.js
 */
export class FileSet {
  constructor() {
    this._group = new Map();
  }
  addEntry(t, e) {
    return this._group.set(t, e || {}), this;
  }
  deleteEntry(t) {
    return this._group.has(t) && this._group.delete(t), this;
  }
  addEntries(t, e) {
    return t.forEach(t => {
      const r = {
        ...e
      };
      this._group.set(t, r || {});
    }), this;
  }
  collect() {
    return this._group;
  }
}
