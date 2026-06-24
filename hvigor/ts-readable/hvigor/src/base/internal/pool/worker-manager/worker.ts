/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/worker-manager/worker.js
 */
import * as worker_state_js_1 from "../enum/worker-state";
export class HWorker {
  constructor(e, t) {
    this.melted = false, this.cacheSize = 0, this.worker = e, this.id = t, this.state = worker_state_js_1.WorkerState.IDLE, this.lastBusyTime = Date.now();
  }
  getState() {
    return this.state;
  }
  getLastBusyTime() {
    return this.lastBusyTime;
  }
  getId() {
    return this.id;
  }
  terminate(e) {
    this.setState(worker_state_js_1.WorkerState.DEAD), this.setLastBusyTime(Date.now()), this.worker.terminate().then(e());
  }
  setState(e) {
    this.state = e;
  }
  setLastBusyTime(e) {
    this.lastBusyTime = e;
  }
  doWork(e) {
    return this.state === worker_state_js_1.WorkerState.IDLE && (this.setState(worker_state_js_1.WorkerState.BUSY), this.curWorkId = e.getId(), this.curTaskInfo = {
      hasSideEffects: e.hasSideEffects(),
      preludeDeps: e.getPreludeDeps()
    }, this.worker.postMessage({
      id: e.getId(),
      workPath: e.getWorkPath(),
      inputData: e.getInputData(),
      exitAfterDone: e.hasSideEffects()
    }), true);
  }
  getCurWorkId() {
    return this.curWorkId;
  }
  cancelCurWorkId() {
    this.curWorkId = undefined;
  }
  isMeltdown() {
    return this.melted;
  }
  meltdown() {
    this.melted = true;
  }
  getCurTaskInfo() {
    return this.curTaskInfo;
  }
  hasCache() {
    return this.cacheSize > 0;
  }
  updateCacheSize(e) {
    this.cacheSize = e;
  }
  sendMessageToWorker(e) {
    this.worker.postMessage(e);
  }
}
