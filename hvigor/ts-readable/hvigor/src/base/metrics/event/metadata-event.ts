/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/event/metadata-event.js
 */
import * as base_event_js_1 from "./base-event";
export var MetadataEventState;
!function (t) {
  t.NEW = "new", t.IDLE = "idle", t.BUSY = "busy", t.CLOSE = "close", t.BROKEN = "broken";
}(MetadataEventState = MetadataEventState || (MetadataEventState = {}));
class MetadataEventAdditional {
  constructor(t) {
    this.state = t;
  }
}
export class MetadataEvent extends base_event_js_1.BaseEvent {
  constructor(t, e, a, s, n, d) {
    super(new base_event_js_1.EventHead(t, e, a, base_event_js_1.MetricEventType.METADATA), new base_event_js_1.EventBody(s, n)), this.additional = new MetadataEventAdditional(d), this.body.startTime = Number(process.hrtime.bigint());
  }
  setCategory(t) {
    this.additional.category = t;
  }
  setSortIndex(t) {
    this.additional.sortIndex = t;
  }
  setLabel(t) {
    this.additional.label = t;
  }
  setContent(t) {
    this.additional.content = t;
  }
}
