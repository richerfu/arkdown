/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/cli/main/daemon-client.js
 */
import process_1 from "process";
import * as index_js_1 from "../../base/boot/index";
import * as build_event_js_1 from "../../base/common/daemon-protocol/build-event";
import * as default_daemon_connector_js_1 from "../../base/daemon/connection/default-daemon-connector";
import * as hvigor_daemon_client_log_js_1 from "../../base/daemon/log/hvigor-daemon-client-log";
import * as global_core_parameters_js_1 from "../../base/internal/data/global-core-parameters";
import pretty_hrtime_js_1 from "../../common/util/pretty-hrtime";
import * as client_event_handler_js_1 from "../handler/client-event-handler";
const log = hvigor_daemon_client_log_js_1.HvigorDaemonClientLogger.getLogger("daemon-client");
export async function daemonBoot(e) {
  const t = process_1.hrtime(),
    n = new default_daemon_connector_js_1.DefaultDaemonConnector(),
    o = await n.getDaemonConnection();
  o ? (o.on(build_event_js_1.BuildEvent.BUILD_STATUS, client_event_handler_js_1.handleBuildStatus), o.on(build_event_js_1.BuildEvent.OUTPUT, client_event_handler_js_1.printLog), o.on(build_event_js_1.WatchEvent.WATCH_RESULT, client_event_handler_js_1.handleWatchBuildResult), o.on(build_event_js_1.WatchEvent.WATCH_LOG, client_event_handler_js_1.handleWatchLog), o.emit(build_event_js_1.BuildEvent.COMMON_BUILD, e), log.debug(`hvigor client: Connection initTime: ${(0, pretty_hrtime_js_1)(process_1.hrtime(t))}`)) : (global_core_parameters_js_1.coreParameter.startParams.daemon = false, log.warn("hvigor client: Failed to find or create an available daemon. Build will be executed in no-daemon mode."), await (0, index_js_1.boot)(e)), process_1.on("exit", e => {
    log.debug("hvigor client: Connection will disconnect and exit process."), null == o || o.disconnect(), process_1.exit(null != e ? e : 0);
  });
}
