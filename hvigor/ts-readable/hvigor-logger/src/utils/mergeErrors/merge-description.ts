/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/merge-description.js
 */
import * as errorlog_consts_js_1 from "../../consts/errorlog-consts";
import * as hvigor_console_logger_js_1 from "../../logger/hvigor-console-logger";
import * as hvigor_error_info_js_1 from "../../logger/hvigor-error-info";
export class MergeDescription {
  constructor() {
    this.log = new hvigor_console_logger_js_1.HvigorConsoleLogger("hvigor");
  }
  mergeTwoKey(r, e, o) {
    let s = "";
    return r || e ? (r || (r = new hvigor_error_info_js_1.HvigorErrorInfo()), e || (e = new hvigor_error_info_js_1.HvigorErrorInfo()), o.code === errorlog_consts_js_1.MergeType.COLLECT_ALL ? s = r.getDescription() + " " + e.getDescription() : o.code === errorlog_consts_js_1.MergeType.COLLECT_FIRST ? s = r.getDescription() : (o.code === errorlog_consts_js_1.MergeType.COLLECT_LAST || this.log.printDebug(`Unknown MergeType: ${o.code}. Use default strategy [COLLECT_LAST].`), s = e.getDescription()), s) : s;
  }
}
