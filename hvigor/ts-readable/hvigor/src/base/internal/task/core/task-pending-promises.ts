/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/task-pending-promises.js
 */
export class TaskPendingPromises {
  constructor() {
    this.promiseSet = new Set();
  }
  add(e) {
    this.promiseSet.add(e);
  }
  get() {
    return [...this.promiseSet];
  }
  clear() {
    this.promiseSet.clear();
  }
}
