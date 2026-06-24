/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/merge-solutions.js
 */
import * as errorlog_consts_js_1 from "../../consts/errorlog-consts";
import * as hvigor_console_logger_js_1 from "../../logger/hvigor-console-logger";
import * as hvigor_error_info_js_1 from "../../logger/hvigor-error-info";
export class MergeSolutions {
  constructor() {
    this.log = new hvigor_console_logger_js_1.HvigorConsoleLogger("hvigor");
  }
  mergeTwoKey(o, r, e) {
    let s = [];
    return o || r ? (o || (o = new hvigor_error_info_js_1.HvigorErrorInfo()), r || (r = new hvigor_error_info_js_1.HvigorErrorInfo()), e.solutions === errorlog_consts_js_1.MergeType.COLLECT_ALL ? s = [...o.getSolutions(), ...r.getSolutions()] : e.solutions === errorlog_consts_js_1.MergeType.COLLECT_FIRST ? s = [...o.getSolutions()] : (e.solutions === errorlog_consts_js_1.MergeType.COLLECT_LAST || this.log.printDebug(`Unknown MergeType: ${e.code}. Use default strategy [COLLECT_LAST].`), s = [...r.getSolutions()]), s) : s;
  }
}
