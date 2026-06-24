/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/worker-pool/worker-pool-impl.js
 */
import * as noop_js_1 from "../../../../common/util/noop";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
import * as duration_event_js_1 from "../../../metrics/event/duration-event";
import * as log_event_js_1 from "../../../metrics/event/log-event";
import * as metric_service_js_1 from "../../../metrics/metric-service";
import * as cpu_js_1 from "../../../util/cpu";
import * as global_core_parameters_js_1 from "../../data/global-core-parameters";
import * as constant_js_1 from "../constant/constant";
import * as dispatcher_factory_js_1 from "../dispatcher/dispatcher-factory";
import * as dispatcher_type_js_1 from "../enum/dispatcher-type";
import * as pool_state_js_1 from "../enum/pool-state";
import * as priority_js_1 from "../enum/priority";
import * as log_service_impl_js_1 from "../log-collection/log-service-impl";
import * as itcb_js_1 from "../model/itcb";
import * as work_js_1 from "../model/work";
import * as cluster_priority_ready_queue_js_1 from "../ready-queue/cluster-priority-ready-queue";
import * as tcb_store_js_1 from "../store/tcb-store";
import * as worker_manager_impl_js_1 from "../worker-manager/worker-manager-impl";
import * as pool_config_js_1 from "./pool-config";
const log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-worker-pool");
export class WorkerPoolImpl {
  constructor(e) {
    log.debug("Worker pool is initialized with config: ", e), this.poolConfig = e, this.resident = true, this.state = pool_state_js_1.PoolState.INIT, this.dispatcher = dispatcher_factory_js_1.DispatcherFactory.getDispatcher(dispatcher_type_js_1.DispatcherType.LOOP_PRIORITY), this.logService = new log_service_impl_js_1.LogServiceImpl(), this.logService.start(), this.workerManager = new worker_manager_impl_js_1.WorkerManagerImpl(e, (e, t) => this.dispatcher.dispatch(this.logService, this.readyQueue, e, t), e => {
      this.logService.addLog(e);
    }), this.readyQueue = new cluster_priority_ready_queue_js_1.ClusterPriorityReadyQueue(this.workerManager.getMaxPoolNum()), (0, cpu_js_1.startMonitor)(), this.state = pool_state_js_1.PoolState.RUNNING, this.checkPoolState() || this.terminate();
  }
  setResident(e) {
    this.resident = e;
  }
  isResident() {
    return this.resident;
  }
  getMaxPoolNum() {
    return this.workerManager.getMaxPoolNum();
  }
  getMinPoolNum() {
    return this.workerManager.getMinPoolNum();
  }
  static getInstance(e) {
    return WorkerPoolImpl.instance && WorkerPoolImpl.instance.state !== pool_state_js_1.PoolState.TERMINATED ? (0, pool_config_js_1.isSamePoolConfig)(WorkerPoolImpl.instance.getPoolConfig(), e) || (log.debug("Current worker pool does not match this config, create a new worker pool."), log.debug("Current pool config: ", WorkerPoolImpl.instance.poolConfig), log.debug("New pool config: ", e), WorkerPoolImpl.instance.terminate().then(e => log.debug(`worker pool terminated with: ${e}`)), WorkerPoolImpl.instance = new WorkerPoolImpl(e)) : (log.debug("Since there is no instance or instance is terminated, create a new worker pool."), WorkerPoolImpl.instance = new WorkerPoolImpl(e)), WorkerPoolImpl.instance;
  }
  updateLogLevel(e) {
    this.workerManager.updateLogLevel(e);
  }
  getLog(e) {
    return this.logService.getLog(e);
  }
  clearLog(e) {
    return this.logService.clear(e);
  }
  getState() {
    return this.state;
  }
  setMaxIdleTime(e) {
    this.workerManager.setMaxIdleTime(e);
  }
  setRecycleInterval(e) {
    this.workerManager.setRecycleInterval(e);
  }
  setErrorCallback(e) {
    this.workerManager.setErrorCallback(e);
  }
  setState(e) {
    pool_state_js_1.transitionMap.get(this.state).has(e) && (this.state = e, this.state === pool_state_js_1.PoolState.STOP && this.readyQueue.clear());
  }
  warmUp(e) {
    if (this.state !== pool_state_js_1.PoolState.RUNNING) return false;
    const t = new work_js_1.Work(priority_js_1.Priority.FIRST, undefined, e),
      r = new itcb_js_1.ITCB(t, noop_js_1.noop, false, [], () => {
        this.readyQueue.update(t);
      }, "", "", "");
    return tcb_store_js_1.TcbStore.add(r), this.readyQueue.push(t), this.dispatcher.dispatch(this.logService, this.readyQueue, this.workerManager), true;
  }
  submit(e, t, r) {
    var o;
    const s = new work_js_1.Work(null !== (o = null == r ? undefined : r.priority) && undefined !== o ? o : priority_js_1.Priority.MEDIUM, this.filterTargetWorkers(null == r ? undefined : r.targetWorkers), t, null == r ? undefined : r.workInput, null == r ? undefined : r.hasSideEffects, null == r ? undefined : r.preludeDeps, null == r ? undefined : r.memorySensitive);
    return this.submitWork(e, s, r);
  }
  filterTargetWorkers(e) {
    if (!e) return e;
    const t = new Set(),
      r = [];
    for (const o of e) Number.isInteger(o) && !t.has(o) && o < this.workerManager.getMaxPoolNum() && o >= 0 && (t.add(o), r.push(o));
    return r;
  }
  submitWork(e, t, r) {
    if (!this.workSubmissionCanBeAccepted(t)) return log.debug(`Task ${e.getPath()}'s work:${t.getId()} will not be executed in worker pool.`), itcb_js_1.ITCB.getDefaultRejectTcb(e, t, r);
    const o = `${e.getName()} work[${t.getId()}]`;
    log.debug(`${o} is submitted.`);
    const s = this.initTcb(e, t, r);
    return this.readyQueue.push(t), log.debug(`${o} is pushed to ready queue.`), this.dispatcher.dispatch(this.logService, this.readyQueue, this.workerManager) ? log.debug(`${o} is dispatched.`) : log.debug(`${o} is not dispatched.`), s;
  }
  workSubmissionCanBeAccepted(e) {
    const t = global_core_parameters_js_1.coreParameter.startParams.parallelExecution,
      r = this.state === pool_state_js_1.PoolState.RUNNING,
      o = undefined === e.getTargetWorkers() || 0 !== e.getTargetWorkers().length;
    return t && r && o;
  }
  initTcb(e, t, r) {
    var o, s;
    const i = new itcb_js_1.ITCB(t, null !== (o = null == r ? undefined : r.callback) && undefined !== o ? o : noop_js_1.noop, null == r ? undefined : r.useReturnVal, null !== (s = null == r ? undefined : r.callbackInput) && undefined !== s ? s : [], () => {
        this.readyQueue.update(t);
      }, e.getNode().getName(), e.getName(), e.getPath()),
      a = e,
      n = new Promise((t, r) => {
        i.on(constant_js_1.PoolConstant.WORK_DONE, r => {
          var o, s;
          log.debug(`${e.getName()} work[${r}] done.`), a.taskExecutedStatus.setWorkerTimePeriod([null !== (o = i.getStartTime()) && undefined !== o ? o : Number(process.hrtime.bigint()), null !== (s = i.getEndTime()) && undefined !== s ? s : Number(process.hrtime.bigint())]), this.recordDurationEvent(a, r, i, true), t(true);
        }), i.on(constant_js_1.PoolConstant.WORK_ERROR, t => {
          log.debug(`${e.getName()} work[${t.id}] failed.`), this.recordDurationEvent(a, t.id, i, false), r(t.error);
        }), i.on(constant_js_1.PoolConstant.CALLBACK_ERROR, t => {
          log.debug(`Callback of work[${t.id}] in ${e.getName()} failed.`), r(t.error);
        });
      });
    return a.pendingPromises.add(n), a.addSubDurationEvent(t.getId()), tcb_store_js_1.TcbStore.add(i), i;
  }
  recordDurationEvent(e, t, r, o) {
    const s = e.getSubDurationEvent(t);
    null == s || s.setTid(`Worker${r.getWorkerId()}`), null == s || s.start(duration_event_js_1.DurationEventState.RUNNING, r.getStartTime()), null == s || s.stop(o ? duration_event_js_1.DurationEventState.SUCCESS : duration_event_js_1.DurationEventState.FAILED, r.getEndTime()), null == s || s.setLog(null == s ? undefined : s.getName(), o ? log_event_js_1.MetricLogType.INFO : log_event_js_1.MetricLogType.ERROR, null == s ? undefined : s.getDescription()), this.updateTid(s);
  }
  updateTid(e) {
    e && e.getChildren().forEach(t => {
      const r = metric_service_js_1.MetricService.getInstance().getEventById(t);
      this.updateTid(null == r ? undefined : r.setTid(e.getTid()));
    });
  }
  terminate() {
    return new Promise((e, t) => {
      this.state === pool_state_js_1.PoolState.TERMINATED && e(true), this.state === pool_state_js_1.PoolState.RUNNING && this.setState(pool_state_js_1.PoolState.STOP), this.state === pool_state_js_1.PoolState.STOP || this.state === pool_state_js_1.PoolState.CLOSE ? (log.debug("Current worker pool is stopped or closed."), this.logService.end(), this.readyQueue.clear(), this.workerManagerClear(e)) : (log.debug("Current worker pool has been terminated."), (0, cpu_js_1.endMonitor)().then(() => e(false)));
    });
  }
  workerManagerClear(e) {
    this.workerManager.clear(() => {
      (0, cpu_js_1.endMonitor)().then(() => {
        this.setState(pool_state_js_1.PoolState.TERMINATED), log.debug("Current worker pool is terminated."), e(true);
      });
    });
  }
  recycle() {
    return new Promise((e, t) => {
      this.readyQueue.clear(), this.workerManager.cleanUp(() => e(true));
    });
  }
  checkPoolState() {
    return !(this.getMaxPoolNum() < 1 || this.getMaxPoolNum() < this.getMinPoolNum());
  }
  getPoolConfig() {
    return this.poolConfig;
  }
}
