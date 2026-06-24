/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/multi-map.js
 */
export class MultiMap {
  constructor() {
    this._nMap = new Map();
  }
  put(e, t) {
    let s = this._nMap.get(e);
    undefined === s && (s = new Set(), this._nMap.set(e, s)), undefined !== t && s.add(t);
  }
  get(e) {
    const t = this._nMap.get(e);
    return undefined === t ? new Set() : t;
  }
  keys() {
    return [...this._nMap.keys()];
  }
  removeAll(e) {
    const t = this._nMap.get(e);
    return this._nMap.delete(e), undefined === t ? new Set() : t;
  }
  remove(e, t) {
    const s = this._nMap.get(e);
    undefined !== s && s.delete(t);
  }
  clear() {
    this._nMap.clear();
  }
}
