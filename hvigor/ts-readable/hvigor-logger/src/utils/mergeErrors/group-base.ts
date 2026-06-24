/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/mergeErrors/group-base.js
 */
import * as hvigor_file_logger_js_1 from "../../logger/hvigor-file-logger";
export class GroupBase {
  constructor(e, i) {
    this.log = new hvigor_file_logger_js_1.HvigorFileLogger("hvigor"), this.hvigorLogList = e, this.mergeKey = i;
  }
  groupByKey(e, i) {
    this.hvigorLogList || (this.log.printWarn("hvigorLogList is undefined. Auto create empty list."), this.hvigorLogList = []), e[this.mergeKey] && (this.log.printWarn(`mergeKey [${this.mergeKey}] can not exits in merge option. Auto delete this key.`), delete e[this.mergeKey]), this._groupByKey(this.hvigorLogList, e, i);
  }
}
