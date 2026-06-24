/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/logger/hvigor-logger-for-console.js
 */
import * as hvigor_logger_js_1 from "./hvigor-logger";
export class HvigorLoggerForConsole extends hvigor_logger_js_1.HvigorLogger {
  constructor(o, g) {
    super(o, g);
  }
  static getLogger(o) {
    return o || (o = "undefined"), HvigorLoggerForConsole.hvigorLoggerCache.has(o) || HvigorLoggerForConsole.hvigorLoggerCache.set(o, new HvigorLoggerForConsole(o)), HvigorLoggerForConsole.hvigorLoggerCache.get(o);
  }
  static getLoggerWithDurationId(o, g) {
    return o || (o = "undefined"), HvigorLoggerForConsole.hvigorLoggerCache.has(o) || HvigorLoggerForConsole.hvigorLoggerCache.set(o, new HvigorLoggerForConsole(o, g)), HvigorLoggerForConsole.hvigorLoggerCache.get(o);
  }
}
HvigorLoggerForConsole.hvigorLoggerCache = new Map();
