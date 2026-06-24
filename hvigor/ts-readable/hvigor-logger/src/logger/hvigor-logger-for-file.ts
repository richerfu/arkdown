/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/logger/hvigor-logger-for-file.js
 */
import * as log4js from "log4js";
import * as hvigor_logger_config_js_1 from "../utils/hvigor-logger-config";
export class HvigorLoggerForFile {
  constructor(e, r) {
    log4js.configure(hvigor_logger_config_js_1.HvigorLoggerConfig.updateConfiguration()), this.fileLogger = log4js.getLogger("debug-file");
  }
  static getLogger(e) {
    return e || (e = "undefined"), HvigorLoggerForFile.hvigorLoggerCache.has(e) || HvigorLoggerForFile.hvigorLoggerCache.set(e, new HvigorLoggerForFile(e)), HvigorLoggerForFile.hvigorLoggerCache.get(e);
  }
  static getLoggerWithDurationId(e, r) {
    return e || (e = "undefined"), HvigorLoggerForFile.hvigorLoggerCache.has(e) || HvigorLoggerForFile.hvigorLoggerCache.set(e, new HvigorLoggerForFile(e, r)), HvigorLoggerForFile.hvigorLoggerCache.get(e);
  }
  debug(e) {
    this.fileLogger.debug(e.getMessage());
  }
  info(e) {
    this.fileLogger.debug(e.getMessage());
  }
  warn(e) {
    this.fileLogger.warn(e.getMessage());
  }
  error(e) {
    this.fileLogger.error(e.getMessage());
  }
  errorAndExit(e) {
    throw new Error(e.getMessage());
  }
}
HvigorLoggerForFile.hvigorLoggerCache = new Map();
