/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/event/mark-event.js
 */
import * as base_event_js_1 from "./base-event";
export var MarkEventType, MarkEventCategory, MarkEventState;
!function (t) {
  t.HISTORY = "history", t.OTHER = "other";
}(MarkEventType = MarkEventType || (MarkEventType = {})), function (t) {
  t.BUILD = "build", t.CLEAN = "clean";
}(MarkEventCategory = MarkEventCategory || (MarkEventCategory = {})), function (t) {
  t.SUCCESS = "success", t.FAILED = "failed", t.RUNNING = "running";
}(MarkEventState = MarkEventState || (MarkEventState = {}));
export class MarkEventTime {
  constructor(t) {
    this.year = t.getFullYear(), this.month = t.getMonth() + 1, this.day = t.getDate(), this.hour = t.getHours(), this.minute = t.getMinutes(), this.second = t.getSeconds();
  }
}
export class MarkEventAdditional {
  constructor() {
    this.time = new MarkEventTime(new Date());
  }
}
export class MarkEvent extends base_event_js_1.BaseEvent {
  constructor(t, e, a, n, s) {
    super(new base_event_js_1.EventHead(t, e, a, base_event_js_1.MetricEventType.MARK), new base_event_js_1.EventBody(n, s)), this.additional = new MarkEventAdditional();
  }
  start(t = MarkEventState.RUNNING, e) {
    this.setState(t), super.setStartTime(e);
  }
  stop(t = MarkEventState.SUCCESS, e) {
    this.additional.state !== MarkEventState.FAILED && this.additional.state !== MarkEventState.SUCCESS && (this.body.endTime = null != e ? e : Number(process.hrtime.bigint()), this.setState(t));
  }
  setMarkType(t) {
    this.additional.markType = t;
  }
  setCategory(t) {
    this.additional.category = t;
  }
  setState(t) {
    this.additional.state = t;
  }
  setHvigorVersion(t) {
    this.additional.hvigorVersion = t;
  }
  setCompleteCommand(t) {
    this.additional.completeCommand = t;
  }
  setNodeVersion(t) {
    this.additional.nodeVersion = t;
  }
}
