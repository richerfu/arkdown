/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/validate-util.js
 */
import * as hvigor_log_js_1 from "../log/hvigor-log";
export class ValidateUtil {
  static validateModule(t) {
    undefined !== t.name && "" !== t.name || this.logger.printErrorExit("THE_REQUIRE_ATTRIBUTE_MODULE_NAME_IS_MISSING"), undefined !== t.srcPath && "" !== t.srcPath || this.logger.printErrorExit("THE_REQUIRE_ATTRIBUTE_MODULE_SRCPATH_IS_MISSING");
  }
}
ValidateUtil.logger = hvigor_log_js_1.HvigorLogger.getLogger(ValidateUtil.name);
