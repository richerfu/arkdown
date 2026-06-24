/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/logger/hvigor-console-logger.js
 */
import * as errorlog_consts_js_1 from "../consts/errorlog-consts";
import * as error_util_js_1 from "../utils/error-util";
import * as merge_error_list_js_1 from "../utils/mergeErrors/merge-error-list";
import * as hvigor_error_common_adapter_js_1 from "./hvigor-error-common-adapter";
import * as hvigor_logger_for_console_js_1 from "./hvigor-logger-for-console";
export class HvigorConsoleLogger {
  constructor(r) {
    this.subSystem = errorlog_consts_js_1.SubSystemEnum.UNKNOWN, this.hvigorErrorCommonAdapter = new hvigor_error_common_adapter_js_1.HvigorErrorCommonAdapter(), this.hvigorLogList = [], this.subSystem = r;
  }
  printError(r) {
    r = error_util_js_1.ErrorUtil.getRealError(r);
    const o = this.hvigorErrorCommonAdapter.convertErrorToLogInfo(r);
    hvigor_logger_for_console_js_1.HvigorLoggerForConsole.getLogger(this.subSystem).error(o);
  }
  printErrorAndExit(r) {
    r = error_util_js_1.ErrorUtil.getRealError(r);
    const o = this.hvigorErrorCommonAdapter.convertErrorToLogInfo(r);
    hvigor_logger_for_console_js_1.HvigorLoggerForConsole.getLogger(this.subSystem).errorAndExit(o);
  }
  printWarn(r) {
    const o = this.hvigorErrorCommonAdapter.convertWarnToLogInfo(r);
    hvigor_logger_for_console_js_1.HvigorLoggerForConsole.getLogger(this.subSystem).warn(o);
  }
  printInfo(r) {
    const o = this.hvigorErrorCommonAdapter.convertInfoToLogInfo(r);
    hvigor_logger_for_console_js_1.HvigorLoggerForConsole.getLogger(this.subSystem).info(o);
  }
  printDebug(r) {
    const o = this.hvigorErrorCommonAdapter.convertDebugToLogInfo(r);
    hvigor_logger_for_console_js_1.HvigorLoggerForConsole.getLogger(this.subSystem).debug(o);
  }
  pushError(r) {
    this.hvigorLogList.push(error_util_js_1.ErrorUtil.getRealError(r));
  }
  printAllError(r) {
    let o = this.hvigorLogList;
    r && (o = r);
    for (let r = 0; r < (null == o ? undefined : o.length); r++) this.printError(o[r]);
    r || (this.hvigorLogList = []);
  }
  printMergedError(r, o) {
    let e = new merge_error_list_js_1.MergeErrorList(this.hvigorLogList, r, o).getMergedErrorList();
    this.printAllError(e), this.hvigorLogList = [];
  }
}
