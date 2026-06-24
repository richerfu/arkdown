/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/util/print-util.js
 */
import process_1 from "process";
import * as common_enum_js_1 from "../../common/daemon-protocol/common-enum";
import * as session_manager_js_1 from "../session/session-manager";
export function overridePrintWrite() {
  process_1.stdout.write = function (e, r) {
    return session_manager_js_1.defaultSessionManager.emit(session_manager_js_1.SESSION_EMIT_LOG, common_enum_js_1.OutputType.STDOUT, e, r), true;
  }, process_1.stderr.write = function (e, r) {
    return session_manager_js_1.defaultSessionManager.emit(session_manager_js_1.SESSION_EMIT_LOG, common_enum_js_1.OutputType.STDERR, e, r), true;
  };
}
