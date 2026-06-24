/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/event/base-event.js
 */
export var MetricEventType;
(function (t) {
  t.DURATION = "duration", t.INSTANT = "instant", t.COUNTER = "counter", t.GAUGE = "gauge", t.OBJECT = "object", t.METADATA = "metadata", t.MARK = "mark", t.LOG = "log", t.CONTINUAL = "continual";
})(MetricEventType = MetricEventType || (MetricEventType = {}));
export class EventHead {
  constructor(t, e, i, s) {
    this.id = t, this.name = e, this.description = i, this.type = s;
  }
}
export class EventBody {
  constructor(t, e) {
    this.pid = t, this.tid = e, this.startTime = Number(process.hrtime.bigint());
  }
}
export class BaseEvent {
  constructor(t, e) {
    this.head = t, this.body = e, this.additional = {};
  }
  setStartTime(t) {
    this.body.startTime = null != t ? t : Number(process.hrtime.bigint());
  }
  setEndTime(t) {
    this.body.endTime = null != t ? t : Number(process.hrtime.bigint());
  }
  setTotalTime(t) {
    this.body.totalTime = t;
  }
  getId() {
    return this.head.id;
  }
  getName() {
    return this.head.name;
  }
  getDescription() {
    return this.head.description;
  }
  setName(t) {
    this.head.name = t;
  }
  getType() {
    return this.head.type;
  }
  setType(t) {
    this.head.type = t;
  }
  getTid() {
    return this.body.tid;
  }
  setTid(t) {
    return this.body.tid = t, this;
  }
}
