/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/queue.js
 */
export class Queue {
  constructor(e) {
    undefined !== e && (this.elements = new Array(e)), this.elements = [], this._capacity = e;
  }
  push(e) {
    return (undefined === this._capacity || this.elements.length !== this._capacity) && (this.elements.push(e), true);
  }
  pop() {
    return this.elements.shift();
  }
  size() {
    return this.elements.length;
  }
  peek() {
    return this.elements[0];
  }
  clear() {
    this.elements = [];
  }
  popPriorTask(e) {
    for (let t = 0; t < this.elements.length; t++) {
      const s = this.elements[t];
      if ("string" != typeof s) return;
      if (e.has(s)) return this.elements.splice(t, 1), s;
    }
  }
  toString() {
    return this.elements.toString();
  }
}
