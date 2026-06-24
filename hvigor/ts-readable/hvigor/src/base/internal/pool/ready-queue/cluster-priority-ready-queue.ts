/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/ready-queue/cluster-priority-ready-queue.js
 */
import * as heap_js_1 from "../../../../common/util/heap";
import * as task_state_js_1 from "../enum/task-state";
import * as tcb_store_js_1 from "../store/tcb-store";
export class ClusterPriorityReadyQueue {
  constructor(e) {
    this.heaps = new Map();
    for (let t = 0; t < Math.floor(e); t++) this.heaps.set(t, new heap_js_1.Heap(function (e, t) {
      return e.getPriority() - t.getPriority();
    }));
  }
  clear() {
    this.heaps.forEach(e => e.clear());
  }
  empty() {
    for (let e = 0; e < this.heaps.size; e++) if (!this.heaps.get(e).empty()) return false;
    return true;
  }
  pop() {
    for (let e = 0; e < this.heaps.size; e++) {
      const t = this.heaps.get(e);
      if (!t.empty()) return t.pop();
    }
  }
  push(e) {
    var t, s;
    const r = e.getTargetWorkers();
    if (r) for (const s of r) null === (t = this.heaps.get(s)) || undefined === t || t.push(e);else for (let t = 0; t < this.heaps.size; t++) null === (s = this.heaps.get(t)) || undefined === s || s.push(e);
  }
  update(e) {
    for (let t = 0; t < this.heaps.size; t++) {
      this.heaps.get(t).update(e);
    }
  }
  getWorkHeap(e) {
    return this.heaps.get(e);
  }
  getFirstWaitingWork(e) {
    var t;
    const s = this.getWorkHeap(e);
    if (s) for (; !s.empty();) {
      const e = s.pop();
      if ((null === (t = tcb_store_js_1.TcbStore.getTCB(e.getId())) || undefined === t ? undefined : t.getState()) === task_state_js_1.TaskState.WAITING) return e;
    }
  }
  pushWork(e, t) {
    var s;
    null === (s = this.heaps.get(t)) || undefined === s || s.push(e);
  }
}
