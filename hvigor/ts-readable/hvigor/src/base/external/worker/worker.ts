/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/worker/worker.js
 */
import * as hvigor_core_js_1 from "../core/hvigor-core";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
const logger = hvigor_log_js_1.HvigorLogger.getLogger("worker");
export function submitWorker(r, e, o, t) {
  const g = hvigor_core_js_1.hvigorCore.getProject(),
    i = g.getName() === r.getNodeNameInternal() ? g : g.findModuleByName(r.getNodeNameInternal());
  i || logger.printErrorExit("FAILED_TO_SUBMIT_WORKER_MODULE_NOT_FOUND");
  const s = i.getTaskContainer().findTask(e);
  s || logger.printErrorExit("FAILED_TO_SUBMIT_WORKER_TASK_NOT_FOUND");
  const _ = s;
  return _.getWorkerPool().submit(_, o, {
    workInput: t.workInput,
    priority: t.priority,
    targetWorkers: t.targetWorkers,
    callback: t.callback,
    callbackInput: t.callbackInput,
    useReturnVal: t.useReturnVal
  });
}
