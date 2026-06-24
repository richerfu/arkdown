/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/event/counter-event.js
 */
import * as base_event_js_1 from "./base-event";
export class CounterEventAdditional {
  constructor(e, t) {
    this.success = null != e ? e : 0, this.failed = null != t ? t : 0;
  }
}
export class CounterEvent extends base_event_js_1.BaseEvent {
  constructor(e, t, n, s, o, r, i) {
    super(new base_event_js_1.EventHead(e, t, n, base_event_js_1.MetricEventType.COUNTER), new base_event_js_1.EventBody(s, o)), this.additional = new CounterEventAdditional(r, i), this.body.startTime = Number(process.hrtime.bigint());
  }
}
