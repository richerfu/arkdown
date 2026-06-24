/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/stack.js
 */
export class Stack {
  constructor() {
    this.stack = [], this.stack = [];
  }
  push(t) {
    return this.stack.push(t);
  }
  pop() {
    return this.isEmpty() ? undefined : this.stack.pop();
  }
  peek() {
    return this.isEmpty() ? undefined : this.stack[this.size() - 1];
  }
  size() {
    return this.stack.length;
  }
  isEmpty() {
    return 0 === this.stack.length;
  }
}
