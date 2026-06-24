/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/event/object-event.js
 */
import * as base_event_js_1 from "./base-event";
export var ObjectEventState;
!function (e) {
  e.NEW = "new", e.SNAPSHOT = "snapshot", e.DESTROY = "destroy";
}(ObjectEventState = ObjectEventState || (ObjectEventState = {}));
export class ObjectEventAdditional {
  constructor(e, t, s) {
    this.objectId = e, this.state = t, this.snapshot = s;
  }
}
export class ObjectEvent extends base_event_js_1.BaseEvent {
  constructor(e, t, s, n, o, a, c, i) {
    super(new base_event_js_1.EventHead(e, t, s, base_event_js_1.MetricEventType.OBJECT), new base_event_js_1.EventBody(n, o)), this.additional = new ObjectEventAdditional(a, c, i), this.body.startTime = Number(process.hrtime.bigint());
  }
}
