/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/event/duration-event.js
 */
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as metric_factory_js_1 from "../metric-factory";
import * as metric_service_js_1 from "../metric-service";
import * as base_event_js_1 from "./base-event";
import * as continual_event_js_1 from "./continual-event";
import * as log_event_1 from "./log-event";
export var DurationEventState;
!function (t) {
  t.CREATED = "created", t.BEGINNING = "beginning", t.RUNNING = "running", t.FAILED = "failed", t.SUCCESS = "success", t.WARN = "warn";
}(DurationEventState = DurationEventState || (DurationEventState = {}));
const MODULE_KEY_SEP = ":",
  TARGET_KEY_SEP = "@";
class DurationEventAdditional {
  constructor(t, e) {
    this.children = [], this.state = DurationEventState.CREATED, this.targetName = "", this.moduleName = "";
    const i = t.indexOf(":");
    if (i > 0) {
      this.moduleName = t.substring(0, i);
      const e = t.indexOf("@");
      e > 0 && (this.targetName = t.substring(i + 1, e));
    }
    this.category = e, this.taskRunReasons = [];
  }
}
export class DurationEvent extends base_event_js_1.BaseEvent {
  constructor(t, e, i, n, s, a) {
    super(new base_event_js_1.EventHead(t, e, i, base_event_js_1.MetricEventType.DURATION), new base_event_js_1.EventBody(n, a)), this.log = hvigor_log_js_1.HvigorLogger.getLogger("DurationEvent"), this.additional = new DurationEventAdditional(e, s);
  }
  start(t = DurationEventState.RUNNING, e) {
    return this.setState(t), super.setStartTime(e), this;
  }
  stop(t = DurationEventState.SUCCESS, e) {
    if (this.additional.state === DurationEventState.FAILED || this.additional.state === DurationEventState.SUCCESS || this.additional.state === DurationEventState.WARN) return this;
    this.body.endTime = null != e ? e : Number(process.hrtime.bigint());
    const i = metric_service_js_1.MetricService.getInstance();
    this.setState(t);
    for (const e of this.additional.children) {
      const n = i.getEventById(e);
      n ? n instanceof DurationEvent ? n.stop(t) : this.log.warn(`Child:'${e}' is not of type DurationEvent.`) : this.log.warn(`Can not getEventById:'${e}' from MetricCacheService.`);
    }
    return this;
  }
  setState(t) {
    this.additional.state = t;
  }
  createSubEvent(t, e) {
    const i = metric_factory_js_1.MetricFactory.createDurationEvent(t, e, "");
    return i.setParent(this.getId()), this.addChild(i.getId()), i;
  }
  addChild(t) {
    this.additional.children.push(t);
  }
  setParent(t) {
    this.additional.parent = t;
  }
  getParent() {
    return this.additional.parent;
  }
  getChildren() {
    return this.additional.children;
  }
  setLog(t, e = log_event_1.MetricLogType.INFO, i, n) {
    const s = metric_factory_js_1.MetricFactory.createLogEvent(null != t ? t : this.head.name, e, this.getTid(), i);
    s.setDurationId(this.getId()), this.additional.logId = s.getId(), s.setStartTime(this.body.startTime), s.setEndTime(this.body.endTime), n && s.setTotalTime(n), this.setParentLog(s), this.setChildrenLog(s);
  }
  setParentLog(t) {
    const e = metric_service_js_1.MetricService.getInstance().getEventById(this.additional.parent);
    if (e instanceof DurationEvent) {
      const i = metric_service_js_1.MetricService.getInstance().getEventById(e.additional.logId);
      i instanceof log_event_1.LogEvent && (i.addChild(t.getId()), t.setParent(i.getId()));
    }
  }
  setChildrenLog(t) {
    this.additional.children.forEach(e => {
      const i = metric_service_js_1.MetricService.getInstance().getEventById(e);
      if (i instanceof DurationEvent || i instanceof continual_event_js_1.ContinualEvent) {
        t.addChild(i.additional.logId);
        const e = metric_service_js_1.MetricService.getInstance().getEventById(i.additional.logId);
        e instanceof log_event_1.LogEvent && i.setParentLog(e);
      }
    });
  }
  setDetail(t) {
    const e = metric_factory_js_1.MetricFactory.createLogEvent(t, log_event_1.MetricLogType.DETAIL, this.getTid());
    e.setDurationId(this.getId()), this.additional.detailId = e.getId();
  }
  setCategory(t) {
    this.additional.category = t;
  }
  addTaskRunReason(t) {
    this.additional.taskRunReasons.push(t);
  }
}
