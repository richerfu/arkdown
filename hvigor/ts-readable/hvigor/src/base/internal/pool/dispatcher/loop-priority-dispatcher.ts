/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/dispatcher/loop-priority-dispatcher.js
 */
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
import * as cpu_js_1 from "../../../util/cpu";
import * as constant_js_1 from "../constant/constant";
import * as log_js_1 from "../log-collection/log";
import * as tcb_store_js_1 from "../store/tcb-store";
const log = hvigor_log_js_1.HvigorLogger.getLogger("workerDispatcher");
export class LoopPriorityDispatcher {
  constructor() {
    this.failedAttempts = 0;
  }
  dispatch(e, t, o, r) {
    if (t.empty()) return false;
    if (cpu_js_1.cpuUsage >= constant_js_1.PoolConstant.CPU_LIMIT && this.failedAttempts < constant_js_1.PoolConstant.MAX_FAIL_ATTEMPTS) return this.failedAttempts++, setTimeout(() => {
      this.dispatch(e, t, o, r);
    }, constant_js_1.PoolConstant.FAIL_DISPATCH_INTERVAL), false;
    this.failedAttempts = 0;
    const s = t;
    if (undefined !== r) return this.dispatchWithGivenWorker(e, s, o, r);
    for (let t = o.getMaxPoolNum(); t >= 0; t--) if (this.dispatchWithGivenWorker(e, s, o, t)) return true;
    return false;
  }
  dispatchWithGivenWorker(e, t, o, r) {
    if (o.isBusyWorker(r)) return log.debug(`A work dispatched to worker[${r}] failed because of worker busy.`), false;
    const s = t.getFirstWaitingWork(r);
    if (!s) return log.debug(`A work dispatched to worker[${r}] failed because unable to get work from ready queue.`), false;
    const i = tcb_store_js_1.TcbStore.getTCB(s.getId()),
      a = null == i ? undefined : i.getTaskName();
    return o.isIdleWorker(r) || o.createWorker(r, s.getPreludeDeps()) ? o.doWork(s, i.getCallback(), i.getCallbackInput(), r) ? (log.debug(`${a} work[${s.getId()}] has been dispatched to worker[${r}].`), tcb_store_js_1.TcbStore.getTCB(s.getId()).isWarmUp() || e.addLog((0, log_js_1.generateScheduleLog)(`${Date.now()}`, s.getId(), r)), true) : (log.debug(`${a} work[${s.getId()}] dispatched to worker[${r}] failed: doWork failed.`), t.pushWork(s, r), false) : (log.debug(`${a} work[${s.getId()}] dispatched to worker[${r}] failed: create worker failed.`), t.pushWork(s, r), false);
  }
}
