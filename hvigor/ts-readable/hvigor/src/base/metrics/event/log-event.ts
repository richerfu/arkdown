/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/event/log-event.js
 */
import * as base_event_js_1 from "./base-event";
export var MetricLogType;
!function (t) {
  t.DEBUG = "debug", t.INFO = "info", t.WARN = "warn", t.ERROR = "error", t.DETAIL = "detail";
}(MetricLogType = MetricLogType || (MetricLogType = {}));
export class LogEventAdditional {
  constructor(t) {
    this.logType = t, this.children = [];
  }
}
export class LogEvent extends base_event_js_1.BaseEvent {
  constructor(t, e, i, n, o, d) {
    super(new base_event_js_1.EventHead(t, e, i, base_event_js_1.MetricEventType.LOG), new base_event_js_1.EventBody(n, o)), this.additional = new LogEventAdditional(d);
  }
  getLogType() {
    return this.additional.logType;
  }
  setLogType(t) {
    this.additional.logType = t;
  }
  getDurationId() {
    return this.additional.durationId;
  }
  setDurationId(t) {
    this.additional.durationId = t;
  }
  getContinualId() {
    return this.additional.continualId;
  }
  setContinualId(t) {
    this.additional.continualId = t;
  }
  addChild(t) {
    t && -1 === this.additional.children.indexOf(t) && this.additional.children.push(t);
  }
  setParent(t) {
    this.additional.parent || (this.additional.parent = t);
  }
}
