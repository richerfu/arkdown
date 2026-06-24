/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/metric-service.js
 */
import * as report_js_1 from "../common/report/report";
import * as report_service_impl_js_1 from "../common/report/report-service-impl";
import * as metric_cache_service_js_1 from "../internal/cache/service/metric-cache-service";
import * as base_event_js_1 from "./event/base-event";
import * as duration_event_js_1 from "./event/duration-event";
import * as log_event_js_1 from "./event/log-event";
export class MetricService {
  constructor() {
    this.metricCacheService = metric_cache_service_js_1.MetricCacheService.getInstance();
  }
  submit(e) {
    this.metricCacheService.add(e);
  }
  getEventById(e) {
    if (e) return this.metricCacheService.get(e);
  }
  queryReport() {
    let e = this.filterDurationEvent(this.metricCacheService.getEvents());
    return e = this.filterLogEvent(e), new report_js_1.Report("events", e);
  }
  filterDurationEvent(e) {
    return e.filter(e => {
      if (e.getType() === base_event_js_1.MetricEventType.DURATION) {
        if (e.additional.state === duration_event_js_1.DurationEventState.CREATED) return false;
      }
      return true;
    });
  }
  filterLogEvent(e) {
    return e.filter(e => {
      if (e.getType() === base_event_js_1.MetricEventType.LOG) {
        const t = e,
          r = this.getEventById(t.additional.durationId);
        if (r && r.additional.state === duration_event_js_1.DurationEventState.CREATED) return false;
        if (t.additional.logType === log_event_js_1.MetricLogType.DETAIL && !r) return false;
      }
      return true;
    });
  }
  clear() {
    this.metricCacheService.close();
  }
  static getInstance() {
    return MetricService.instance || (MetricService.instance = new MetricService(), report_service_impl_js_1.ReportServiceImpl.getInstance().addListener(MetricService.instance)), MetricService.instance;
  }
}
