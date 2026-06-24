/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/log/hvigor-rollup-logger.js
 */
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
import * as arkts_base_log_1 from "./arkts-base-log";
export class HvigorRollupLogger {
  constructor(r, o) {
    this.subSystem = hvigor_logger_1.SubSystemEnum.UNKNOWN, this.hvigorErrorCommonAdapter = new hvigor_logger_1.HvigorErrorCommonAdapter(), this.hvigorLogList = [], this.mergeLogList = [], this.subSystem = r, this.logEventType = o;
  }
  printError(r) {
    r = hvigor_logger_1.ErrorUtil.getRealError(r);
    const o = this.hvigorErrorCommonAdapter.convertErrorToLogInfo(r);
    arkts_base_log_1.ArktsBaseLogger.addLogEvent(this.getLogEvent("error", o.getMessage()));
  }
  printErrorAndExit(r) {
    r = hvigor_logger_1.ErrorUtil.getRealError(r);
    const o = this.hvigorErrorCommonAdapter.convertErrorToLogInfo(r);
    throw new ArkTsCompilerError(o.getMessage());
  }
  printWarn(r) {
    const o = this.hvigorErrorCommonAdapter.convertWarnToLogInfo(r);
    arkts_base_log_1.ArktsBaseLogger.addLogEvent(this.getLogEvent("warn", o.getMessage()));
  }
  printInfo(r) {
    const o = this.hvigorErrorCommonAdapter.convertInfoToLogInfo(r);
    arkts_base_log_1.ArktsBaseLogger.addLogEvent(this.getLogEvent("info", o.getMessage()));
  }
  printDebug(r) {
    const o = this.hvigorErrorCommonAdapter.convertDebugToLogInfo(r);
    arkts_base_log_1.ArktsBaseLogger.addLogEvent(this.getLogEvent("debug", o.getMessage()));
  }
  pushError(r, o = true) {
    this.hvigorLogList.push(hvigor_logger_1.ErrorUtil.getRealError(r)), o && this.mergeLogList.push(hvigor_logger_1.ErrorUtil.getRealError(r));
  }
  printAllError(r) {
    let o = this.hvigorLogList;
    r && (o = r);
    for (let r = 0; r < o?.length; r++) this.printError(o[r]);
    r || (this.hvigorLogList = [], this.mergeLogList = []);
  }
  printMergedError(r, o) {
    let e = new hvigor_logger_1.MergeErrorList(this.hvigorLogList, r, o).getMergedErrorList();
    this.printAllError(e);
  }
  getLogEvent(r, o) {
    return {
      time: new Date(),
      category: this.subSystem,
      level: r,
      msg: o,
      type: this.logEventType
    };
  }
}
class ArkTsCompilerError extends Error {
  constructor(r, o) {
    super(r), this.name = ArkTsCompilerError.ARKTS_COMPILER_ERROR_NAME, o && (this.stack = o);
  }
}
ArkTsCompilerError.ARKTS_COMPILER_ERROR_NAME = "ArkTSCompilerError";
