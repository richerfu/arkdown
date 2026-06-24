/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/metric-factory.js
 */
import crypto_1 from "crypto";
import * as continual_event_js_1 from "./event/continual-event";
import * as counter_event_js_1 from "./event/counter-event";
import * as duration_event_js_1 from "./event/duration-event";
import * as gauge_event_js_1 from "./event/gauge-event";
import * as instant_event_js_1 from "./event/instant-event";
import * as log_event_js_1 from "./event/log-event";
import * as mark_event_js_1 from "./event/mark-event";
import * as metadata_event_js_1 from "./event/metadata-event";
import * as object_event_js_1 from "./event/object-event";
import * as metric_service_js_1 from "./metric-service";
export const MAIN_THREAD = "Main Thread";
export class MetricFactory {
  static getUuid() {
    return crypto_1.randomUUID();
  }
  static createDurationEvent(e, t, r, n) {
    const s = new duration_event_js_1.DurationEvent(MetricFactory.getUuid(), e, t, process.pid, r, null != n ? n : MAIN_THREAD);
    return metric_service_js_1.MetricService.getInstance().submit(s), s;
  }
  static createInstantEvent(e, t, r) {
    const n = new instant_event_js_1.InstantEvent(MetricFactory.getUuid(), e, t, process.pid, null != r ? r : MAIN_THREAD);
    return metric_service_js_1.MetricService.getInstance().submit(n), n;
  }
  static createCounterEvent(e, t, r, n, s) {
    const c = new counter_event_js_1.CounterEvent(MetricFactory.getUuid(), e, t, process.pid, null != s ? s : MAIN_THREAD, r, n);
    return metric_service_js_1.MetricService.getInstance().submit(c), c;
  }
  static createGaugeEvent(e, t, r, n) {
    const s = new gauge_event_js_1.GaugeEvent(MetricFactory.getUuid(), e, r, process.pid, null != n ? n : MAIN_THREAD, t);
    return metric_service_js_1.MetricService.getInstance().submit(s), s;
  }
  static createObjectEvent(e, t, r, n, s, c) {
    const i = new object_event_js_1.ObjectEvent(MetricFactory.getUuid(), e, n, process.pid, null != c ? c : MAIN_THREAD, t, r, s);
    return metric_service_js_1.MetricService.getInstance().submit(i), i;
  }
  static createMetadataEvent(e, t, r, n) {
    const s = new metadata_event_js_1.MetadataEvent(MetricFactory.getUuid(), e, r, process.pid, null != n ? n : MAIN_THREAD, t);
    return metric_service_js_1.MetricService.getInstance().submit(s), s;
  }
  static createMarkEvent(e, t, r) {
    const n = new mark_event_js_1.MarkEvent(MetricFactory.getUuid(), e, t, process.pid, null != r ? r : MAIN_THREAD);
    return metric_service_js_1.MetricService.getInstance().submit(n), n;
  }
  static createLogEvent(e, t, r, n) {
    const s = new log_event_js_1.LogEvent(MetricFactory.getUuid(), e, null != n ? n : "", process.pid, null != r ? r : MAIN_THREAD, t);
    return metric_service_js_1.MetricService.getInstance().submit(s), s;
  }
  static createContinualEvent(e, t, r, n, s) {
    const c = new continual_event_js_1.ContinualEvent(MetricFactory.getUuid(), e, t, process.pid, null != s ? s : MAIN_THREAD, r, n);
    return metric_service_js_1.MetricService.getInstance().submit(c), c;
  }
}
