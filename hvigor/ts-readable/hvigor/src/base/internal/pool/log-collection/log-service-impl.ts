/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/log-collection/log-service-impl.js
 */
import * as report_js_1 from "../../../common/report/report";
import * as report_service_impl_js_1 from "../../../common/report/report-service-impl";
import * as log_type_js_1 from "../enum/log-type";
import * as log_collector_js_1 from "./log-collector";
export class LogServiceImpl {
  constructor() {
    this.isActive = true, this.logCollectorMap = new Map(), this.logCollectorMap.set(log_type_js_1.LogType.SCHEDULE, new log_collector_js_1.LogCollector()), this.logCollectorMap.set(log_type_js_1.LogType.WORK, new log_collector_js_1.LogCollector()), report_service_impl_js_1.ReportServiceImpl.getInstance().addListener(this);
  }
  clear(e) {
    var o;
    e ? null === (o = this.logCollectorMap.get(e)) || undefined === o || o.clear() : this.logCollectorMap.clear();
  }
  getLog(e) {
    var o;
    return null === (o = this.logCollectorMap.get(e)) || undefined === o ? undefined : o.dump();
  }
  addLog(e) {
    var o;
    this.isActive && (null === (o = this.logCollectorMap.get(e.type)) || undefined === o || o.add(e));
  }
  start() {
    this.isActive = true;
  }
  end() {
    this.isActive = false;
  }
  queryReport() {
    return new report_js_1.Report(`${log_type_js_1.LogType.WORK}Log`, new Set());
  }
}
