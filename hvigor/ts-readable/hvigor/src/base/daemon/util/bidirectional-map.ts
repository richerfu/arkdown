/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/util/bidirectional-map.js
 */
export class BidirectionalMap {
  constructor() {
    this.forwardMap = new Map(), this.reverseMap = new Map();
  }
  getByKey(e) {
    return this.forwardMap.get(e);
  }
  getByValue(e) {
    return this.reverseMap.get(e);
  }
  checkByKey(e) {
    return undefined !== e && this.forwardMap.has(e);
  }
  checkByValue(e) {
    return undefined !== e && this.reverseMap.has(e);
  }
  set(e) {
    this.forwardMap.set(e[0], e[1]), this.reverseMap.set(e[1], e[0]);
  }
  deleteByKey(e) {
    this.checkByKey(e) && (this.reverseMap.delete(this.getByKey(e)), this.forwardMap.delete(e));
  }
  deleteByValue(e) {
    this.checkByValue(e) && (this.forwardMap.delete(this.getByValue(e)), this.reverseMap.delete(e));
  }
}
