/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/worker-pool/worker-delegator.js
 */
import * as worker_pool_factory_js_1 from "./worker-pool-factory";
export class WorkerPoolDelegator {
  constructor() {
    let o;
    this.workerPool = () => (WorkerPoolDelegator.isActive = true, null != o ? o : o = worker_pool_factory_js_1.WorkerPoolFactory.getDefaultWorkerPool());
  }
  warmUp(o) {
    if (WorkerPoolDelegator.needWarmUp) {
      const e = this.workerPool().getMinPoolNum();
      for (let r = 0; r < e; r++) this.workerPool().warmUp(o);
      WorkerPoolDelegator.needWarmUp = false;
    }
  }
  submit(o, e, r) {
    return this.workerPool().submit(o, e, r);
  }
  async terminate() {
    const o = await this.workerPool().terminate();
    return WorkerPoolDelegator.isActive = !o, o;
  }
  getState() {
    return this.workerPool().getState();
  }
  setState(o) {
    this.workerPool().setState(o);
  }
  setRecycleInterval(o) {
    this.workerPool().setRecycleInterval(o);
  }
  setMaxIdleTime(o) {
    this.workerPool().setMaxIdleTime(o);
  }
  setErrorCallback(o) {
    this.workerPool().setErrorCallback(o);
  }
  getLog(o) {
    return this.workerPool().getLog(o);
  }
  clearLog(o) {
    this.workerPool().clearLog(o);
  }
  getMaxPoolNum() {
    return this.workerPool().getMaxPoolNum();
  }
  getMinPoolNum() {
    return this.workerPool().getMinPoolNum();
  }
  setResident(o) {
    this.workerPool().setResident(o);
  }
  isActive() {
    return WorkerPoolDelegator.isActive;
  }
}
WorkerPoolDelegator.isActive = false, WorkerPoolDelegator.needWarmUp = false;
