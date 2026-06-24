/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/task-execute-status.js
 */
export const TASK_EXECUTE_STATUS = {
  BLOCKED: 0,
  PENDING: 1,
  RUNNING: 2,
  END: 3,
  CANCELLED: 4,
  FAILED: 5
};
export class TaskExecuteStatus {
  constructor() {
    this._workerTimePeriod = [], this._isUpToDate = false, this._state = TASK_EXECUTE_STATUS.BLOCKED;
  }
  setState(t) {
    if (t < this._state) throw new Error("Cannot change the task status.");
    this._state = t, t === TASK_EXECUTE_STATUS.RUNNING && (this._taskBeginTime = process.hrtime());
  }
  getState() {
    return this._state;
  }
  getTaskBeginTime() {
    return this._taskBeginTime;
  }
  setIsUpToDate(t) {
    this._isUpToDate = t;
  }
  isUpToDate() {
    return this._isUpToDate;
  }
  setWorkerTimePeriod(t) {
    this._workerTimePeriod.push(t);
  }
  getWorkerTimePeriod() {
    return this._workerTimePeriod;
  }
}
