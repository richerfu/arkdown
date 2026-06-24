/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/log/hvigor-daemon-client-log.js
 */
import path_1 from "path";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import log4js_1 from "log4js";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as log_event_js_1 from "../../metrics/event/log-event";
import * as daemon_const_js_1 from "../const/daemon-const";
const daemonConfiguration = {
  appenders: {
    "daemon-client-file": {
      type: "fileSync",
      filename: path_1.resolve(daemon_const_js_1.DAEMON_LOG_PATH, "daemon-client.log"),
      maxLogSize: 10485760,
      backups: 2,
      encoding: "utf-8"
    }
  },
  categories: {
    "daemon-client": {
      appenders: ["daemon-client-file", "just-debug", "just-info", "just-wrong"],
      level: "debug"
    }
  }
};
export class HvigorDaemonClientLogger extends hvigor_log_js_1.HvigorLogger {
  constructor(e) {
    super("default"), this._daemonClientFileLogger = log4js_1.getLogger(e);
  }
  static getLogger(e) {
    return this.getInstance(HvigorDaemonClientLogger, e);
  }
  log(e, ...o) {
    this.createLogEventByDurationId(e, log_event_js_1.MetricLogType.INFO, ...o), this._daemonClientFileLogger.log(e, ...o);
  }
  debug(e, ...o) {
    this.createLogEventByDurationId(e, log_event_js_1.MetricLogType.DEBUG, ...o), this._daemonClientFileLogger.debug(e, ...o);
  }
  info(e, ...o) {
    this.createLogEventByDurationId(e, log_event_js_1.MetricLogType.INFO, ...o), this._daemonClientFileLogger.info(e, ...o);
  }
  warn(e, ...o) {
    undefined !== e && "" !== e && (this.createLogEventByDurationId(e, log_event_js_1.MetricLogType.WARN, ...o), this._daemonClientFileLogger.warn(e, ...o));
  }
  error(e, ...o) {
    this.createLogEventByDurationId(e, log_event_js_1.MetricLogType.ERROR, ...o), this._daemonClientFileLogger.error(e, ...o);
  }
  clientErrorLink() {
    this.error(`A log of client process can be found in: ${path_1.resolve(daemon_const_js_1.DAEMON_LOG_PATH, "daemon-client.log")}`);
  }
  daemonErrorLink(e) {
    let o = "00306056 Specification Limit Violation";
    o += `\nA log of daemon process can be found at file: ${path_1.resolve(daemon_const_js_1.DAEMON_LOG_PATH, `daemon-${e}.log`)}`, o += "\n\n* Try the following:\n  > Increase the daemon memory size by setting nodeOptions.maxOldSpaceSize.", o += "\n  > Run the build with '--no-daemon' to verify whether the issue is daemon-related.";
    o += `\n  > More info about hvigor daemon: ${"cn" === (0, hvigor_common_1.getOsLanguage)() ? "https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/ide-hvigor-daemon" : "https://developer.huawei.com/consumer/en/doc/harmonyos-guides/ide-hvigor-daemon"}`, this.error(o);
  }
  clientWarnLink() {
    this.warn(`A log of client process can be found in: ${path_1.resolve(daemon_const_js_1.DAEMON_LOG_PATH, "daemon-client.log")}`);
  }
  daemonWarnLink(e) {
    this.warn(`A log of daemon process can be found in: ${path_1.resolve(daemon_const_js_1.DAEMON_LOG_PATH, `daemon-${e}.log`)}`);
  }
}
export function configureDaemonClient() {
  (0, hvigor_log_js_1.configure)(daemonConfiguration);
}
