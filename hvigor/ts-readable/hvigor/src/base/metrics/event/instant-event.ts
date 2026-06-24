/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/event/instant-event.js
 */
import * as base_event_js_1 from "./base-event";
export var InstantEventScope;
!function (t) {
  t.THREAD = "thread", t.PROCESS = "process", t.GLOBAL = "global";
}(InstantEventScope = InstantEventScope || (InstantEventScope = {}));
export class InstantEventAdditional {}
export class InstantEvent extends base_event_js_1.BaseEvent {
  constructor(t, e, n, s, a) {
    super(new base_event_js_1.EventHead(t, e, n, base_event_js_1.MetricEventType.INSTANT), new base_event_js_1.EventBody(s, a)), this.additional = new InstantEventAdditional(), this.body.startTime = Number(process.hrtime.bigint());
  }
  setScope(t) {
    this.additional.scope = t;
  }
}
