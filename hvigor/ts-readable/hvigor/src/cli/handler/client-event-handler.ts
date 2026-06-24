/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/cli/handler/client-event-handler.js
 */
import process_1 from "process";
import util_1 from "util";
import * as common_enum_js_1 from "../../base/common/daemon-protocol/common-enum";
import * as hvigor_log_js_1 from "../../base/log/hvigor-log";
const log = hvigor_log_js_1.HvigorLogger.getLogger("client-event-handler");
export function printLog(e) {
  e && e.type && e.text ? (e.type === common_enum_js_1.OutputType.STDOUT && process_1.stdout.write(e.text, e.encoding), e.type === common_enum_js_1.OutputType.STDERR && process_1.stderr.write(e.text, e.encoding)) : handleServerMsgError(log, e);
}
export function handleBuildStatus(e) {
  function t(t) {
    var r;
    log.debug(t), process_1.exitCode = null !== (r = e.exitCode) && undefined !== r ? r : -1, process_1.exit();
  }
  e && e.status ? (e.reason && log.error(e.reason), e.status === common_enum_js_1.BuildStatus.FINISH && t("The build request was finished"), e.status === common_enum_js_1.BuildStatus.REJECT && t("The build request was rejected."), e.status === common_enum_js_1.BuildStatus.CLOSE && t("The build request was closed.")) : handleServerMsgError(log, e);
}
export function handleWatchBuildResult(e) {
  log.debug(e);
}
export function handleWatchLog(e) {
  log.info(e);
}
export function handleServerMsgError(e, t) {
  e.warn("Received server exception type information."), e.warn(`receive server message:${util_1.format(t)}`);
}
