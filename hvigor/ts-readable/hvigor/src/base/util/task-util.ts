/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/task-util.js
 */
import * as const_js_1 from "../../common/const/const";
import * as hvigor_core_js_1 from "../external/core/hvigor-core";
import * as hvigor_js_1 from "../external/core/hvigor";
import * as incremental_exec_task_js_1 from "../external/task/incremental-exec-task";
import * as incremental_task_js_1 from "../external/task/incremental-task";
import * as task_proxy_manager_js_1 from "../internal/task/proxy/task-proxy-manager";
import * as hvigor_log_js_1 from "../log/hvigor-log";
const log = hvigor_log_js_1.HvigorLogger.getLogger("task-util");
export function isIncrementalTask(e) {
  var t;
  if (null !== (t = hvigor_core_js_1.hvigorCore.getParameter().getPropertyInternal(const_js_1.INCREMENTAL_INPUT_OUTPUT_CACHE)) && undefined !== t && t) return e instanceof incremental_task_js_1.IncrementalTask;
  {
    if (!(e instanceof incremental_task_js_1.IncrementalTask)) return false;
    const t = e.declareInputs(),
      r = e.declareInputFiles(),
      s = e.declareOutputFiles(),
      a = 0 === t.size && 0 === r.collect().size && 0 === s.collect().size;
    return e instanceof incremental_exec_task_js_1.IncrementalExecTask ? !("" === e.declareExecutionTool() && "" === e.declareExecutionCommand() && 0 === e.declareExecutionEnv().size && a) : (a && log.debug(`${e.getPath()} is declared as IncrementTask, but does not implement any incremental method.`), !a);
  }
}
export function markTaskExecuteSuccessful(e) {
  var t, r;
  e.getEnabled() && e.taskShouldDo() && isIncrementalTask(e) && (null === (r = null === (t = task_proxy_manager_js_1.taskProxyManager.getTaskProxy(e.getPath())) || undefined === t ? undefined : t.getCurTaskSnapShot()) || undefined === r || r.setExecutedSuccessful());
}
export function getAlignTarget() {
  return hvigor_js_1.hvigor.getParameterInternal().getPropertyInternal("ohos.align.target");
}
