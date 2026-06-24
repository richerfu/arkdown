/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/core/incremental-task-executor.js
 */
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as task_proxy_js_1 from "../task/proxy/task-proxy";
import * as task_proxy_manager_js_1 from "../task/proxy/task-proxy-manager";
import * as global_core_parameters_js_1 from "../data/global-core-parameters";
const log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-incremental-task");
export async function doIncrementalTask(e) {
  const t = e,
    a = task_proxy_manager_js_1.taskProxyManager.set(t.getPath(), new task_proxy_js_1.TaskProxy(t));
  if (function () {
    if (!global_core_parameters_js_1.coreParameter.startParams.incrementalExecution) return true;
    const t = e.taskExecutedStatus.unTrackStateReason;
    if (t) return log.debug(`${e.getPath()} will not track state,eg do incremental,because:${t}`), true;
    return false;
  }()) t.declareInputs(), t.declareInputFiles(), await a.execute();else {
    a.preExecute() ? t.taskExecutedStatus.setIsUpToDate(true) : await a.execute();
  }
}
