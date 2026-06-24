/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/model/itcb.js
 */
import * as events_1 from "events";
import * as noop_js_1 from "../../../../common/util/noop";
import * as task_state_js_1 from "../enum/task-state";
export class ITCB extends events_1.EventEmitter {
  constructor(t, e, s, a, i, r, n, o, u = false, h) {
    super(), this.customActions = new Map(), this.work = t, this.cb = e, this.cbInput = a, this.submitTime = Date.now(), this.updateLoc = i, this.taskPath = r, this.taskName = n, this.taskCompletePath = o, this.state = null != h ? h : task_state_js_1.TaskState.WAITING, this.useRetVal = null != s && s, this.warmUp = u;
  }
  static getDefaultRejectTcb(t, e, s) {
    return new ITCB(e, noop_js_1.noop, null == s ? undefined : s.useReturnVal, [], noop_js_1.noop, t.getNode().getName(), t.getName(), t.getPath(), false, task_state_js_1.TaskState.REJECT);
  }
  getId() {
    return this.work.getId();
  }
  getWorkerId() {
    return this.workerId;
  }
  getSubmitTime() {
    return this.submitTime;
  }
  getStartTime() {
    return this.startTime;
  }
  getEndTime() {
    return this.endTime;
  }
  getState() {
    return this.state;
  }
  getPriority() {
    return this.work.getPriority();
  }
  getCallback() {
    return this.cb;
  }
  getCallbackInput() {
    return this.cbInput;
  }
  setWorkerId(t) {
    this.workerId = t;
  }
  setState(t) {
    var e;
    return !!(null === (e = task_state_js_1.transitionMap.get(this.state)) || undefined === e ? undefined : e.has(t)) && (this.state = t, true);
  }
  setPriority(t) {
    return this.state === task_state_js_1.TaskState.WAITING && (this.work.setPriority(t), this.updateLoc(), true);
  }
  setCallback(t, e) {
    return (this.state === task_state_js_1.TaskState.WAITING || this.state === task_state_js_1.TaskState.RUNNING) && (this.cb = t, this.cbInput = null != e ? e : this.cbInput, true);
  }
  setCustomAction(t, e) {
    return (this.state === task_state_js_1.TaskState.WAITING || this.state === task_state_js_1.TaskState.RUNNING) && (this.customActions.set(t, e), true);
  }
  popCustomAction(t) {
    const e = this.customActions.get(t);
    return this.customActions.delete(t), e;
  }
  setStartTime(t) {
    this.startTime = t;
  }
  setEndTime(t) {
    this.endTime = t;
  }
  getTaskCompletePath() {
    return this.taskCompletePath;
  }
  getTaskName() {
    return this.taskName;
  }
  getTaskPath() {
    return this.taskPath;
  }
  useReturnVal() {
    return this.useRetVal;
  }
  isWarmUp() {
    return this.warmUp;
  }
}
