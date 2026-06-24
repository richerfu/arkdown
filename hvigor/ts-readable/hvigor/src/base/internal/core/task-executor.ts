/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/core/task-executor.js
 */
import process_1 from "process";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as time_util_js_1 from "../../util/time-util";
import * as hvigor_process_js_1 from "../lifecycle/hvigor-process";
import * as task_control_center_js_1 from "../task/core/task-control-center";
import * as ts_check_js_1 from "../util/ts_check";
import * as global_core_parameters_js_1 from "../data/global-core-parameters";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-task-executor");
export class TaskExecutor {
  async initTasks(e) {
    task_control_center_js_1.taskControlCenter.initTaskQueueBeforeRun(e), global_core_parameters_js_1.coreParameter.startParams.hvigorfileTypeCheck && (0, ts_check_js_1.hvigorfileTypeCheck)(), logConfigurationCostTime();
  }
}
function logConfigurationCostTime() {
  const e = process_1.hrtime(hvigor_process_js_1.hvigorProcess.taskBeginTime),
    r = (0, time_util_js_1.formatTime)(e);
  _log.debug(`Configuration task cost before running: ${r}`);
}
export const taskExecutorInst = new TaskExecutor();
