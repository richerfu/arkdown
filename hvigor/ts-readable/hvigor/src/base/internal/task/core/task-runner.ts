/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/task-runner.js
 */
import process_1 from "process";
import * as wrap_js_1 from "../../../incremental-decorator/wrap";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
import * as task_util_js_1 from "../../../util/task-util";
import * as time_util_js_1 from "../../../util/time-util";
import * as incremental_task_executor_js_1 from "../../core/incremental-task-executor";
import * as event_id_options_js_1 from "../../lifecycle/event/event-id-options";
import * as hvigor_process_js_1 from "../../lifecycle/hvigor-process";
import * as task_module_util_js_1 from "../util/task-module-util";
import * as task_path_util_js_1 from "../util/task-path-util";
import * as task_control_center_listener_js_1 from "./listener/task-control-center-listener";
import * as task_control_center_js_1 from "./task-control-center";
import * as task_execute_status_js_1 from "./task-execute-status";
import * as incremental_task_js_1 from "../../../external/task/incremental-task";
import * as task_proxy_js_1 from "../proxy/task-proxy";
import * as task_proxy_manager_js_1 from "../proxy/task-proxy-manager";
import * as hvigor_core_js_1 from "../../../external/core/hvigor-core";
import * as const_js_1 from "../../../../common/const/const";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("task-runner"),
  runTaskFromQueue = async () => {
    var e;
    const t = true === hvigor_core_js_1.hvigorCore.getParameter().getPropertyInternal(const_js_1.INCREMENTAL_OPTIMIZATION) && task_control_center_js_1.taskControlCenter.popPriorTask() || task_control_center_js_1.taskControlCenter.popRunnableTask();
    if (!t) return void (task_control_center_js_1.taskControlCenter.allTasksHasExecuted() && hvigor_process_js_1.hvigorProcess.emit(event_id_options_js_1.HVIGOR_PROCESS_EVENT_ID.FINISHED));
    const s = (0, task_module_util_js_1.getModuleFromTaskPath)(t);
    if (!s) {
      const e = `Cannot find module from task ${t}!`;
      return _log.error(e), void hvigor_process_js_1.hvigorProcess.emit(event_id_options_js_1.HVIGOR_PROCESS_EVENT_ID.FAILED, e);
    }
    const r = (0, task_path_util_js_1.getTaskNameFromTaskPath)(t),
      o = s.getTaskContainer().findTask(r);
    if (undefined === o) {
      const e = `Cannot find task ${t}!`;
      return _log.error(e), void hvigor_process_js_1.hvigorProcess.emit(event_id_options_js_1.HVIGOR_PROCESS_EVENT_ID.FAILED, e);
    }
    hvigor_process_js_1.hvigorProcess.hasTaskDone = true, o.taskExecutedStatus.setState(task_execute_status_js_1.TASK_EXECUTE_STATUS.PENDING);
    if (!(await executeOneTask(o))) return;
    const _ = !!o.pendingPromises.get().length;
    if (o.getEnabled()) {
      if (o.taskExecutedStatus.isUpToDate()) (0, task_control_center_listener_js_1.onTaskUpToDated)(o);else if (!_) {
        const t = process_1.hrtime(hvigor_process_js_1.hvigorProcess.taskBeginTime),
          s = (0, time_util_js_1.formatTime)(t);
        _log.debug(`runTaskFromQueue task cost before running: ${s}`), await (null === (e = o.executeAfterRun) || undefined === e ? undefined : e.bind(o)()), (0, task_control_center_listener_js_1.onTaskFinished)(o);
      }
    } else (0, task_control_center_listener_js_1.onTaskDisabled)(o);
    _ && handleTaskPendingPromises(o), await (0, runTaskFromQueue)();
  };
const handleTaskPendingPromises = e => {
  const t = e.pendingPromises.get();
  e.pendingPromises.clear(), t.forEach(t => {
    t.catch(t => {
      (0, task_control_center_listener_js_1.onWorkFailed)(e, t);
    });
  }), Promise.all(t).catch(() => Promise.reject("failed")).then(async () => {
    var t;
    return await (null === (t = e.executeAfterRun) || undefined === t ? undefined : t.bind(e)()), (0, task_control_center_listener_js_1.onTaskFinished)(e), (0, runTaskFromQueue)();
  }).catch(e => {
    e instanceof Error && hvigor_process_js_1.hvigorProcess.error(e);
  });
};
async function executeOneTask(e) {
  try {
    const t = process_1.memoryUsage();
    if (e.taskExecutedStatus.setState(task_execute_status_js_1.TASK_EXECUTE_STATUS.RUNNING), e.durationEvent.start(), task_control_center_js_1.taskControlCenter.pushRunningTask(e), !e.getEnabled()) return e.taskExecutedStatus.setState(task_execute_status_js_1.TASK_EXECUTE_STATUS.END), true;
    if (_log.debug(`Executing task :${e.getPath()}`), e instanceof incremental_task_js_1.IncrementalTask && (await e.beforeAlwaysAction(), (0, wrap_js_1.wrapIncrementalDeclarations)(e)), (0, task_util_js_1.isIncrementalTask)(e)) {
      const t = e;
      moduleSkipCondition(t) ? (task_proxy_manager_js_1.taskProxyManager.set(t.getPath(), new task_proxy_js_1.TaskProxy(t)), t.taskExecutedStatus.setIsUpToDate(true)) : await (0, incremental_task_executor_js_1.doIncrementalTask)(e);
    } else await e.execute();
    e.taskExecutedStatus.setState(task_execute_status_js_1.TASK_EXECUTE_STATUS.END);
    const s = process_1.memoryUsage();
    _log.debug(e.getNode().getName(), ":", e.getName(), "cost memory", (s.heapUsed - t.heapUsed) / 1024 / 1024);
    const r = (0, time_util_js_1.formatTimeToNumber)(e.taskExecutedStatus.getTaskBeginTime());
    return e.taskExecutedStatus.setWorkerTimePeriod([r, Number(process_1.hrtime.bigint())]), true;
  } catch (t) {
    return !(t instanceof Error) || ((0, task_control_center_listener_js_1.onTaskFailed)(e, t), hvigor_process_js_1.hvigorProcess.error(t, e.getTaskLog()), false);
  }
}
function shouldSkipModule(e) {
  var t;
  const s = e.getPath().split(":")[0];
  if (hvigor_core_js_1.hvigorCore.getChangedModuleNameList().has(s)) return false;
  const r = null === (t = hvigor_core_js_1.hvigorCore.getModuleDepRelationMap()) || undefined === t ? undefined : t.get(s);
  if (r) for (const e of r) if (hvigor_core_js_1.hvigorCore.getChangedModuleNameList().has(e)) return false;
  return true;
}
function moduleSkipCondition(e) {
  var t;
  return "true" === process_1.env.ENABLE_MODULE_SKIP && 0 !== hvigor_core_js_1.hvigorCore.getModuleDepRelationMap().size && undefined === (null === (t = e.taskExecutedStatus) || undefined === t ? undefined : t.unTrackStateReason) && !!shouldSkipModule(e);
}
export { runTaskFromQueue };
