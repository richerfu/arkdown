/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/logger/hvigor-log-info.js
 */
import * as errorlog_consts_js_1 from "../consts/errorlog-consts";
export class HvigorLogInfo {
  constructor(e) {
    this.message = "", this.logLevel = errorlog_consts_js_1.LogLevel.DEBUG, e && (this.message = e.message, this.logLevel = e.logLevel);
  }
  setMessage(e) {
    return this.message = e, this;
  }
  setLogLevel(e) {
    return this.logLevel = e, this;
  }
  getMessage() {
    return this.message;
  }
  getLogLevel() {
    return this.logLevel;
  }
}
