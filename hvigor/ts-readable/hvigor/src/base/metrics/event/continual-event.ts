/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/event/continual-event.js
 */
import * as metric_factory_js_1 from "../metric-factory";
import * as metric_service_js_1 from "../metric-service";
import * as base_event_js_1 from "./base-event";
import * as duration_event_js_1 from "./duration-event";
import * as log_event_js_1 from "./log-event";
export class ContinualEventAdditional {
  constructor(t, e) {
    this.totalTime = null != t ? t : 0, this.frequency = null != e ? e : 0, this.children = [];
  }
}
export class ContinualEvent extends base_event_js_1.BaseEvent {
  constructor(t, e, n, i, s, a, r) {
    super(new base_event_js_1.EventHead(t, e, n, base_event_js_1.MetricEventType.CONTINUAL), new base_event_js_1.EventBody(i, s)), this.additional = new ContinualEventAdditional(a, r);
  }
  setParent(t) {
    this.additional.parent = t;
  }
  getParent() {
    return this.additional.parent;
  }
  addChild(t) {
    this.additional.children.push(t);
  }
  getChildren() {
    return this.additional.children;
  }
  createSubEvent(t, e) {
    const n = metric_factory_js_1.MetricFactory.createContinualEvent(t, e);
    return n.setParent(this.getId()), this.addChild(n.getId()), n;
  }
  setLog(t, e, n) {
    const i = metric_factory_js_1.MetricFactory.createLogEvent(t, e, this.getTid(), n);
    i.setContinualId(this.getId()), this.additional.logId = i.getId(), i.setStartTime(this.body.startTime), i.setEndTime(this.body.endTime), this.setParentLog(i), this.setChildrenLog(i);
  }
  setParentLog(t) {
    const e = metric_service_js_1.MetricService.getInstance().getEventById(this.additional.parent);
    if (e instanceof ContinualEvent || e instanceof duration_event_js_1.DurationEvent) {
      const n = metric_service_js_1.MetricService.getInstance().getEventById(e.additional.logId);
      n instanceof log_event_js_1.LogEvent && (n.addChild(t.getId()), t.setParent(n.getId()));
    }
  }
  setDetail(t) {
    const e = metric_factory_js_1.MetricFactory.createLogEvent(t, log_event_js_1.MetricLogType.DETAIL, this.getTid());
    e.setContinualId(this.getId()), this.additional.detailId = e.getId();
  }
  setChildrenLog(t) {
    this.additional.children.forEach(e => {
      const n = metric_service_js_1.MetricService.getInstance().getEventById(e);
      if (n instanceof ContinualEvent) {
        t.addChild(n.additional.logId);
        const e = metric_service_js_1.MetricService.getInstance().getEventById(n.additional.logId);
        e instanceof log_event_js_1.LogEvent && n.setParentLog(e);
      }
    });
  }
}
