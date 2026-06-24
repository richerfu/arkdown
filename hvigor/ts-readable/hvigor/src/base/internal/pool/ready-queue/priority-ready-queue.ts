/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/ready-queue/priority-ready-queue.js
 */
import * as heap_js_1 from "../../../../common/util/heap";
import * as priority_js_1 from "../enum/priority";
export class PriorityReadyQueue {
  constructor() {
    this.heap = new heap_js_1.Heap(function (e, r) {
      return e.getPriority() - r.getPriority();
    });
  }
  clear() {
    this.heap.clear();
  }
  pop() {
    return this.heap.pop();
  }
  push(e) {
    if (e.getPriority() === priority_js_1.Priority.FIRST && this.hasFirstWork()) {
      const r = this.heap.pop();
      this.heap.push(e), this.heap.push(r);
    } else this.heap.push(e);
  }
  update(e) {
    this.heap.update(e);
  }
  empty() {
    return this.heap.empty();
  }
  hasFirstWork() {
    return !this.heap.empty() && this.heap.peek().getPriority() === priority_js_1.Priority.FIRST;
  }
}
