/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/merge-code.js
 */
import * as errorlog_consts_js_1 from "../../consts/errorlog-consts";
import * as hvigor_console_logger_js_1 from "../../logger/hvigor-console-logger";
import * as hvigor_error_info_js_1 from "../../logger/hvigor-error-info";
export class MergeCode {
  constructor() {
    this.log = new hvigor_console_logger_js_1.HvigorConsoleLogger("hvigor");
  }
  mergeTwoKey(e, o, r) {
    let g = "";
    return e || o ? (e || (e = new hvigor_error_info_js_1.HvigorErrorInfo()), o || (o = new hvigor_error_info_js_1.HvigorErrorInfo()), r.code === errorlog_consts_js_1.MergeType.COLLECT_ALL ? g = e.getCode() + " " + o.getCode() : r.code === errorlog_consts_js_1.MergeType.COLLECT_FIRST ? g = e.getCode() : (r.code === errorlog_consts_js_1.MergeType.COLLECT_LAST || this.log.printDebug(`Unknown MergeType: ${r.code}. Use default strategy [COLLECT_LAST].`), g = o.getCode()), g) : g;
  }
}
