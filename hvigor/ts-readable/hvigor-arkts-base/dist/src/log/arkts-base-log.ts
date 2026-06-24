/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/log/arkts-base-log.js
 */
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
import * as constants_js_1 from "../util/constants";
import * as hvigor_rollup_logger_js_1 from "./hvigor-rollup-logger";
export class LogCore {
  constructor(e, r) {
    this._debugLog = [], this._infoLog = [], this._warnLog = [], this._errorLog = [], this._log = [], this.colorCodes = {
      warn: "[33m",
      error: "[31m",
      info: "[34m",
      reset: "[39m"
    }, this._category = e, this._type = r;
  }
  getDebugLogs() {
    return this._debugLog;
  }
  debug(...e) {
    this.logMsg(this._debugLog, this.getLogEvent("debug", e));
  }
  getInfoLogs() {
    return this._infoLog;
  }
  info(...e) {
    this.logMsg(this._infoLog, this.getLogEvent("info", e));
  }
  getWarnLogs() {
    return this._warnLog;
  }
  warn(...e) {
    this.logMsg(this._warnLog, this.getLogEvent("warn", e));
  }
  getErrorLogs() {
    return this._errorLog;
  }
  error(...e) {
    this.logMsg(this._errorLog, this.getLogEvent("error", e));
  }
  getLogs() {
    return this._log;
  }
  clear() {
    this._log.length = 0, this._debugLog.length = 0, this._infoLog.length = 0, this._warnLog.length = 0, this._errorLog.length = 0;
  }
  logMsg(e, r) {
    e.push(r), this._log.push(r), ArktsBaseLogger.addLogEvent(r);
  }
  getLogEvent(e, r) {
    return {
      time: new Date(),
      category: this._category,
      level: e,
      msg: (r => r.map(r => "object" == typeof r && this.colorCodes[e] ? this.colorCodes[e] + JSON.stringify(r) + this.colorCodes.reset : "string" == typeof r && this.colorCodes[e] ? this.colorCodes[e] + r + this.colorCodes.reset : r).join(" "))(r),
      type: this._type
    };
  }
}
export class ArktsBaseLogger {
  static setLogCallback(e, r) {
    ArktsBaseLogger.sendLog = e, ArktsBaseLogger.tsWatchCb = r;
  }
  static addLastLogEvents(e) {
    ArktsBaseLogger.lastLogEvents.push(...e);
  }
  static getLastLogEvents() {
    return ArktsBaseLogger.lastLogEvents;
  }
  static getPluginLogger(e) {
    return ArktsBaseLogger.getLogger(e, constants_js_1.PLUGIN_LOG_TYPE);
  }
  static getHvigorConsoleLogger(e) {
    return ArktsBaseLogger.getHvigorLogger(e, constants_js_1.PLUGIN_LOG_TYPE);
  }
  static getHvigorLogger(e, r = constants_js_1.NORMAL_LOG_TYPE) {
    return ArktsBaseLogger._logMapRollup.has(e) || ArktsBaseLogger._logMapRollup.set(e, new hvigor_rollup_logger_js_1.HvigorRollupLogger(e, r)), ArktsBaseLogger._logMapRollup.get(e);
  }
  static getLogger(e, r = constants_js_1.NORMAL_LOG_TYPE) {
    return ArktsBaseLogger._logMap.has(e) || ArktsBaseLogger._logMap.set(e, new LogCore(e, r)), ArktsBaseLogger._logMap.get(e);
  }
  static getLogMap() {
    return ArktsBaseLogger._logMap;
  }
  static addLogEvent(e) {
    if (ArktsBaseLogger.checkTsWatchEnd(e) && ArktsBaseLogger.tsWatchCb) return ArktsBaseLogger.tsWatchCb();
    ArktsBaseLogger._logEvents.push(e), "error" === e.level && (this._logEventsHasError = true), ArktsBaseLogger.sendLog && ArktsBaseLogger.sendLog([e]);
  }
  static getLogEvents() {
    return ArktsBaseLogger._logEvents;
  }
  static checkLoggerHaveErrorInfo() {
    return this._logEventsHasError;
  }
  static sendLastLogEvents(e) {
    ArktsBaseLogger.sendLog && ArktsBaseLogger.sendLog(ArktsBaseLogger.lastLogEvents, e);
  }
  static clearLogEvents() {
    for (const e of ArktsBaseLogger._logEvents) "string" != typeof e.msg || "etsChecker" === e.category || "warn" !== e.level && "error" !== e.level || ArktsBaseLogger.lastLogEvents.push(e);
    ArktsBaseLogger._logEvents.length = 0, this._logEventsHasError = false;
  }
  static recordErrorFileId(e) {
    this.errorFileIds.add(e);
  }
  static getErrorFileIds() {
    return this.errorFileIds;
  }
  static checkTsWatchEnd(e) {
    return "string" == typeof e.msg && "TS Watch End" === e.msg;
  }
  static reset() {
    ArktsBaseLogger._logMap.clear(), ArktsBaseLogger.sendLog = undefined, ArktsBaseLogger.tsWatchCb = undefined, ArktsBaseLogger.lastLogEvents = [], ArktsBaseLogger.errorAdaptor = undefined, ArktsBaseLogger.errorFileIds.clear();
  }
  static clearLogMessages() {
    for (const e of ArktsBaseLogger._logMap.values()) e.clear();
  }
  static setError(e) {
    ArktsBaseLogger.errorAdaptor = undefined, e.type && "none" !== e.type && hvigor_logger_1.MATCH_FIELD_TYPES.includes(e.type) ? ArktsBaseLogger.errorAdaptor = new hvigor_logger_1.ArkTsErrorAdaptor(e["checkMessage" === e.type ? "message" : e.type], "none") : ArktsBaseLogger.errorAdaptor = new hvigor_logger_1.ArkTsErrorAdaptor(e, "none");
  }
}
ArktsBaseLogger._logMap = new Map(), ArktsBaseLogger._logMapRollup = new Map(), ArktsBaseLogger._logEvents = [], ArktsBaseLogger._logEventsHasError = false, ArktsBaseLogger.sendLog = undefined, ArktsBaseLogger.tsWatchCb = undefined, ArktsBaseLogger.lastLogEvents = [], ArktsBaseLogger.errorFileIds = new Set();
