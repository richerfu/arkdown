/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/listener/task-control-center-listener.js
 */
import * as hvigor_trace_js_1 from "../../../../common/trace/hvigor-trace";
import * as hvigor_log_js_1 from "../../../../log/hvigor-log";
import * as duration_event_js_1 from "../../../../metrics/event/duration-event";
import * as log_event_1 from "../../../../metrics/event/log-event";
import * as task_util_js_1 from "../../../../util/task-util";
import * as time_util_js_1 from "../../../../util/time-util";
import * as transform_hvigor_task_js_1 from "../../../../vigor/task/impl/transform-hvigor-task";
import * as hvigor_process_js_1 from "../../../lifecycle/hvigor-process";
import * as task_path_util_js_1 from "../../util/task-path-util";
import * as task_control_center_js_1 from "../task-control-center";
const log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-task-control-center-listener"),
  onWorkFailed = (t, e) => {
    (0, onTaskFailed)(t, e), hvigor_process_js_1.hvigorProcess.error(e, t.getTaskLog());
  };
export const onTaskFinished = t => {
  t.durationEvent.stop(duration_event_js_1.DurationEventState.SUCCESS), (0, task_util_js_1.markTaskExecuteSuccessful)(t);
  const e = process.hrtime(t.taskExecutedStatus.getTaskBeginTime()),
    o = (0, time_util_js_1.formatTime)((0, time_util_js_1.formatTimeToNumPair)((0, time_util_js_1.getIntersect)(t.taskExecutedStatus.getWorkerTimePeriod()))),
    s = "" === o ? (0, time_util_js_1.formatTime)(e) : o,
    r = `Finished :${t.getPath()}`;
  t.durationEvent.setLog(r, log_event_1.MetricLogType.INFO, t.getDescription(), (0, time_util_js_1.getIntersect)(t.taskExecutedStatus.getWorkerTimePeriod())), log._printTaskExecuteInfo(t.getPath(), s), trace(t, e), task_control_center_js_1.taskControlCenter.setTaskExecuted(t.getPath());
};
function trace(t, e) {
  const o = (0, task_path_util_js_1.getModuleNameFromTaskPath)(t.getPath());
  t instanceof transform_hvigor_task_js_1.TransformHvigorTask ? hvigor_trace_js_1.hvigorTrace.traceCustomTaskTime(t.getName(), o, 1e9 * e[0] + e[1]) : hvigor_trace_js_1.hvigorTrace.traceTaskTime(t.getName(), o, 1e9 * e[0] + e[1]);
}
export const onTaskFailed = (t, e) => {
  t.durationEvent.stop(duration_event_js_1.DurationEventState.FAILED);
  const o = `Failed :${t.getPath()}`;
  t.durationEvent.setLog(o, log_event_1.MetricLogType.ERROR, t.getDescription()), log._printFailedTaskInfo(t.getPath()), t.onFailed(e);
};
export const onTaskDisabled = t => {
  t.durationEvent.stop(duration_event_js_1.DurationEventState.SUCCESS);
  const e = `Disabled :${t.getPath()}`;
  t.durationEvent.setLog(e, log_event_1.MetricLogType.INFO, t.getDescription()), log._printDisabledTaskInfo(t.getPath()), task_control_center_js_1.taskControlCenter.setTaskExecuted(t.getPath());
};
export const onTaskUpToDated = t => {
  t.durationEvent.stop(duration_event_js_1.DurationEventState.SUCCESS);
  const e = `UP-TO-DATE :${t.getPath()}`;
  t.durationEvent.setLog(e, log_event_1.MetricLogType.INFO, t.getDescription()), (0, task_util_js_1.markTaskExecuteSuccessful)(t), log._printUpToDateTaskInfo(t.getPath()), task_control_center_js_1.taskControlCenter.setTaskExecuted(t.getPath());
};
export { onWorkFailed };
