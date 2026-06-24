/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/event/gauge-event.js
 */
import * as base_event_js_1 from "./base-event";
export class GaugeEventAdditional {
  constructor(e) {
    this.utilization = e;
  }
}
export class GaugeEvent extends base_event_js_1.BaseEvent {
  constructor(e, t, s, n, a, i) {
    super(new base_event_js_1.EventHead(e, t, s, base_event_js_1.MetricEventType.GAUGE), new base_event_js_1.EventBody(n, a)), this.additional = new GaugeEventAdditional(i), this.body.startTime = Number(process.hrtime.bigint());
  }
}
