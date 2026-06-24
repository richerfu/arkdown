/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/log/hvigor-daemon-log.js
 */
import path_1 from "path";
import log4js_1 from "log4js";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as log_event_js_1 from "../../metrics/event/log-event";
import * as daemon_const_js_1 from "../const/daemon-const";
const daemonConfiguration = {
  appenders: {
    "daemon-file": {
      type: "fileSync",
      filename: path_1.resolve(daemon_const_js_1.DAEMON_LOG_PATH, `daemon-${process.pid}.log`),
      maxLogSize: 10485760,
      backups: 2,
      encoding: "utf-8"
    }
  },
  categories: {
    daemon: {
      appenders: ["daemon-file"],
      level: "debug"
    }
  }
};
export class HvigorDaemonLogger extends hvigor_log_js_1.HvigorLogger {
  constructor(e) {
    super("default"), this._daemonFileLogger = log4js_1.getLogger(e);
  }
  static getLogger(e) {
    return this.getInstance(HvigorDaemonLogger, e);
  }
  log(e, ...o) {
    this.createLogEventByDurationId(e, log_event_js_1.MetricLogType.INFO, ...o), this._daemonFileLogger.log(e, ...o);
  }
  debug(e, ...o) {
    this.createLogEventByDurationId(e, log_event_js_1.MetricLogType.DEBUG, ...o), this._daemonFileLogger.debug(e, ...o);
  }
  info(e, ...o) {
    this.createLogEventByDurationId(e, log_event_js_1.MetricLogType.INFO, ...o), this._daemonFileLogger.info(e, ...o);
  }
  warn(e, ...o) {
    undefined !== e && "" !== e && (this.createLogEventByDurationId(e, log_event_js_1.MetricLogType.WARN, ...o), this._daemonFileLogger.warn(e, ...o));
  }
  error(e, ...o) {
    this.createLogEventByDurationId(e, log_event_js_1.MetricLogType.ERROR, ...o), this._daemonFileLogger.error(e, ...o);
  }
}
export function configureDaemon() {
  (0, hvigor_log_js_1.configure)(daemonConfiguration);
}
