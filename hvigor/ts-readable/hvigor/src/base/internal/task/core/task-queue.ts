/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/task-queue.js
 */
import * as queue_js_1 from "../../../../common/util/queue";
export class TaskQueue {
  constructor(e) {
    this.taskQueue = new queue_js_1.Queue(e);
  }
  push(e) {
    return this.taskQueue.push(e);
  }
  pop() {
    return this.taskQueue.pop();
  }
  peek() {
    return this.taskQueue.peek();
  }
  size() {
    return this.taskQueue.size();
  }
  toString() {
    return this.taskQueue.toString();
  }
  isEmpty() {
    return 0 === this.taskQueue.size();
  }
  clear() {
    this.taskQueue.clear();
  }
  popPriorTask(e) {
    return this.taskQueue.popPriorTask(e);
  }
}
