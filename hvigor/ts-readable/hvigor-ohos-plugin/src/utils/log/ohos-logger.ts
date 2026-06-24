/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/log/ohos-logger.js
 */
import os_1 from "os";
import util_1 from "util";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
import * as log4js_1 from "log4js";
export class OhosLogger extends hvigor_1.HvigorLogger {
  constructor(e) {
    super(e), this.messageMap = new Map(), this.errList = [];
  }
  static getLogger(e) {
    return this.getInstance(OhosLogger, e);
  }
  static getLoggerWithDurationId(e, s) {
    const t = {
        ...super.getInstance(OhosLogger, e)
      },
      r = Object.setPrototypeOf(t, OhosLogger.prototype);
    return r.durationId = s, r;
  }
  _buildError(e, s) {
    return s && this.messageMap.set("code", s), this.messageMap.set("cause", e), this;
  }
  _solution(e) {
    return this.messageMap.set("solution", e), this;
  }
  _detail(e) {
    return this.messageMap.set("solution", e), this;
  }
  _help(e) {
    return this.messageMap.set("help", e), this;
  }
  _record() {
    const e = Array.from(this.messageMap.values()),
      s = util_1.format(this._formatLog("root"), ...e);
    this.messageMap.clear(), this.errList.push(s);
  }
  _printAllExit() {
    if (0 === this.errList.length) return;
    for (let e = 0; e < this.errList.length - 1; e++) this.error(this.errList[e]);
    const e = this.errList[this.errList.length - 1];
    this.errList = [], this.errorMessageExit(e);
  }
  _format() {
    const e = Array.from(this.messageMap.values()),
      s = util_1.format(this._formatLog("root"), ...e);
    return this.messageMap.clear(), s;
  }
  _file(e, s = -1, t = -1) {
    let r = `at ${e}`;
    return s > 0 && (r += `:${s}`, t > 0 && (r += `:${t}`)), this.messageMap.set("file", r), this;
  }
  _callstack(e) {
    return this.getLevel() === log4js_1.levels.DEBUG && this.messageMap.set("callstack", e), this;
  }
  _printDebugCommand(e, s) {
    this.debug(`Use tool [${e}]\n`, s);
  }
  _printWarn(e) {
    const s = Array.from(this.messageMap.values());
    this.warn(this._formatLog(e), ...s);
  }
  _printError(e) {
    const s = Array.from(this.messageMap.values());
    this.error(this._formatLog(e), ...s);
  }
  _printErrorAndExit(e = "root") {
    const s = Array.from(this.messageMap.values());
    this.errorMessageExit(this._formatLog(e), ...s);
  }
  _formatLog(e) {
    let s = this.messageMap.has("code") ? "[%s]%s" : "%s";
    for (const e of this.messageMap.keys()) switch (e) {
      case "solution":
        s += `${os_1.EOL}\t Detail: %s`;
        break;
      case "help":
        s += `${os_1.EOL}\t Help: %s`;
        break;
      case "file":
        s += `${os_1.EOL}\t %s`;
        break;
      case "callstack":
        s += `${os_1.EOL + os_1.EOL}    %s`;
    }
    return this.messageMap.clear(), s;
  }
  getAdaptor(e) {
    return new hvigor_logger_1.HvigorOhosPluginAdaptor(e);
  }
}
export const handleLogs = (e, s, t) => {
  let r = "",
    o = "",
    a = "";
  const i = "[39m";
  e.filter(e => t.isLessThanOrEqualTo(e.level)).forEach(e => {
    "warn" === e.level && (r += `[33mWARN: ${e.msg}${i}${os_1.EOL}`), "info" === e.level && (o += `[34mINFO: ${e.msg}${i}${os_1.EOL}`), "debug" === e.level && (a += `DEBUG: ${e.msg}${os_1.EOL}`);
  }), "" !== r && s.warn(r), "" !== o && s.info(o), "" !== a && s.debug(a);
};
