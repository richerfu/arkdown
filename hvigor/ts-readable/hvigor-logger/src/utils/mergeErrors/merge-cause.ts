/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/merge-cause.js
 */
import * as errorlog_consts_js_1 from "../../consts/errorlog-consts";
import * as hvigor_console_logger_js_1 from "../../logger/hvigor-console-logger";
import * as hvigor_error_info_js_1 from "../../logger/hvigor-error-info";
export class MergeCause {
  constructor() {
    this.log = new hvigor_console_logger_js_1.HvigorConsoleLogger("hvigor");
  }
  mergeTwoKey(e, r, o) {
    let s = "";
    return e || r ? (e || (e = new hvigor_error_info_js_1.HvigorErrorInfo()), r || (r = new hvigor_error_info_js_1.HvigorErrorInfo()), o.cause === errorlog_consts_js_1.MergeType.COLLECT_ALL ? s = e.getCause() + errorlog_consts_js_1.SPLIT_TAG + r.getCause() : o.cause === errorlog_consts_js_1.MergeType.COLLECT_FIRST ? s = e.getCause() : o.cause === errorlog_consts_js_1.MergeType.COLLECT_LAST ? s = r.getCause() : (this.log.printDebug(`Unknown MergeType: ${o.code}. Use default strategy [COLLECT_LAST].`), s = r.getCode()), s) : s;
  }
}
