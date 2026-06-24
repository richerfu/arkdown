/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/logger/hvigor-logger.js
 */
import * as log4js from "log4js";
import * as hvigor_logger_config_js_1 from "../utils/hvigor-logger-config";
export class HvigorLogger {
  constructor(e, g) {
    log4js.configure(hvigor_logger_config_js_1.HvigorLoggerConfig.updateConfiguration()), this.consoleLogger = log4js.getLogger(e), this.setLevel(hvigor_logger_config_js_1.HvigorLoggerConfig.getLevel()), this.fileLogger = log4js.getLogger("debug-file");
  }
  getFileLogger() {
    return this.fileLogger;
  }
  getConsoleLogger() {
    return this.consoleLogger;
  }
  debug(e) {
    undefined !== e.getMessage() && "" !== e.getMessage() && (this.consoleLogger.debug(e.getMessage()), this.fileLogger.debug(e.getMessage()));
  }
  info(e) {
    undefined !== e.getMessage() && "" !== e.getMessage() && (this.consoleLogger.info(e.getMessage()), this.fileLogger.info(e.getMessage()));
  }
  warn(e) {
    undefined !== e.getMessage() && "" !== e.getMessage() && (this.consoleLogger.warn(e.getMessage()), this.fileLogger.warn(e.getMessage()));
  }
  error(e) {
    this.consoleLogger.error(e.getMessage()), this.fileLogger.error(e.getMessage());
  }
  errorAndExit(e) {
    throw this.consoleLogger.error(e.getMessage()), this.fileLogger.error(e.getMessage()), new Error(e.getMessage());
  }
  errorWithoutStack(e) {
    this.consoleLogger.error(e.getMessage()), process.exit(-1);
  }
  setLevel(e) {
    this.consoleLogger.level = e;
  }
}
