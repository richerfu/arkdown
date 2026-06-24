/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/worker-manager/worker-manager-impl.js
 */
import path_1 from "path";
import * as worker_threads_1 from "worker_threads";
import * as index_js_1 from "../../../../common/util/iconv/index";
import * as noop_js_1 from "../../../../common/util/noop";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
import * as time_util_js_1 from "../../../util/time-util";
import * as global_core_parameters_js_1 from "../../data/global-core-parameters";
import * as constant_js_1 from "../constant/constant";
import * as log_type_js_1 from "../enum/log-type";
import * as task_state_js_1 from "../enum/task-state";
import * as worker_state_js_1 from "../enum/worker-state";
import * as tcb_store_js_1 from "../store/tcb-store";
import * as worker_js_1 from "./worker";
import * as extended_error_info_js_1 from "./extended-error-info";
export class WorkerManagerImpl {
  constructor(e, t, r) {
    this._log = hvigor_log_js_1.HvigorLogger.getLogger(WorkerManagerImpl.name), this.hvigorMemoryThreshold = global_core_parameters_js_1.coreParameter.properties.hvigorMemoryThreshold, this.errorCallback = noop_js_1.noop, this.maxPoolNum = Math.floor(e.maxPoolNum && e.maxPoolNum >= 1 ? e.maxPoolNum : constant_js_1.PoolConstant.MAX_POOL_NUM), this.minPoolNum = Math.floor(e.minPoolNum && e.minPoolNum >= 0 ? Math.max(e.minPoolNum, constant_js_1.PoolConstant.MIN_POOL_NUM) : constant_js_1.PoolConstant.MIN_POOL_NUM), this.minPoolNum = Math.min(this.minPoolNum, this.maxPoolNum), this.recycleInterval = Math.floor(e.recycleInterval && e.recycleInterval > 0 ? e.recycleInterval : constant_js_1.PoolConstant.DEFAULT_RECYCLE_INTERVAL), this.maxIdleTime = Math.floor(e.maxIdleTime && e.maxIdleTime > 0 ? e.maxIdleTime : constant_js_1.PoolConstant.DEFAULT_MAX_IDLE_TIME), this.cacheTtl = e.cacheTtl, this.timer = setInterval(() => this.recycle(), this.recycleInterval), this.idleWorkers = new Map(), this.busyWorkers = new Map(), this.deadWorkers = new Set(), this.residentWorkers = new Set(), this.callbacks = new Map(), this.eventMap = new Map([[constant_js_1.PoolConstant.WORK_DONE, task_state_js_1.TaskState.END], [constant_js_1.PoolConstant.WORK_ERROR, task_state_js_1.TaskState.ERROR]]), this.dispatch = t, this.addLog = r, this.maxCoreSize = undefined !== e.maxCoreSize && e.maxCoreSize >= 0 ? e.maxCoreSize : undefined, this.maxCoreSize && (this.maxCoreSize = Math.floor(this.maxCoreSize)), this.cacheCapacity = e.cacheCapacity;
    for (let e = 0; e < this.minPoolNum; e++) this.createWorker();
  }
  getMaxPoolNum() {
    return this.maxPoolNum;
  }
  getMinPoolNum() {
    return this.minPoolNum;
  }
  updateLogLevel(e) {
    const t = t => {
      t.forEach((t, r) => {
        this.residentWorkers.has(r) && t.sendMessageToWorker({
          event: constant_js_1.WorkerEvent.UPDATE_LOG_LEVEL,
          logLevel: e
        });
      });
    };
    t(this.busyWorkers), t(this.idleWorkers), this._log.debug(`Workers' log level updated to ${e}`);
  }
  cleanUp(e) {
    const t = t => {
      t.forEach((r, s) => {
        if (this.residentWorkers.has(s) && !this.busyWorkers.has(s) && r.hasCache()) return this.cleanHvigorConfig(r), this.cleanHvigorLogger(r), void r.sendMessageToWorker({
          event: constant_js_1.WorkerEvent.UNMOUNT_TSC_COMMON_CACHE_EVENT
        });
        this._log.debug(`Cleanup worker ${s}.`), t.delete(s), r.terminate(() => {
          this._log.debug(`Worker ${s} has been cleaned up.`), this._log.debug(`Current idle worker size: ${this.idleWorkers.size}.`), this._log.debug(`Current resident worker size: ${this.residentWorkers.size}.`), this.idleWorkers.size === this.residentWorkers.size && e();
        });
      });
    };
    this.busyWorkers.size + this.idleWorkers.size === 0 ? (this._log.debug("There's no busy workers and idle workers need cleanup."), this.callbacks.clear(), e()) : (t(this.busyWorkers), t(this.idleWorkers), this.callbacks.clear());
  }
  clear(e) {
    clearInterval(this.timer), this.busyWorkers.size + this.idleWorkers.size === 0 ? (this._log.debug("There's no busy workers and idle workers need to be cleared."), this.deadWorkers.clear(), this.residentWorkers.clear(), this.callbacks.clear(), e()) : (this.clearWorkers(e, this.idleWorkers), this.clearWorkers(e, this.busyWorkers), this.callbacks.clear());
  }
  cleanHvigorConfig(e) {
    e.sendMessageToWorker({
      event: constant_js_1.WorkerEvent.CLEAN_HVIGOR_CONFIG_LOADER
    });
  }
  cleanHvigorLogger(e) {
    e.sendMessageToWorker({
      event: constant_js_1.WorkerEvent.CLEAN_HVIGOR_LOGGER
    });
  }
  clearWorkers(e, t) {
    t.forEach((r, s) => {
      this._log.debug(`Clear worker ${s}.`), t.delete(s), r.terminate(() => {
        this._log.debug(`Worker ${s} has been cleared.`), this._log.debug(`Current idle worker size: ${this.idleWorkers.size}.`), this._log.debug(`Current busy worker size: ${this.busyWorkers.size}.`), this.idleWorkers.size || this.busyWorkers.size || (this.deadWorkers.clear(), this.residentWorkers.clear(), e());
      });
    });
  }
  createWorker(e, t = []) {
    if (this.isFull()) return this._log.debug("Failed to create worker since the worker pool is full."), false;
    const r = this.getAllocatedId(e);
    if (r < 0) return this._log.debug("Failed to create worker since no worker id could be allocated."), false;
    const s = undefined === this.maxCoreSize || this.residentWorkers.size < this.maxCoreSize;
    this._log.debug(`Create ${s ? "" : "non-"} resident worker with id: ${r}.`);
    const o = this._createWorker(t, s),
      i = new worker_js_1.HWorker(o, r);
    return this.idleWorkers.set(i.getId(), i), s && this.residentWorkers.add(r), this.listenWorkerEvents(o, i, r), o.stdout.on("data", this.getDataHandler(i)(process.stdout)), o.stderr.on("data", this.getDataHandler(i)(process.stderr)), true;
  }
  listenWorkerEvents(e, t, r) {
    this.listenExitEvent(e, t, r), this.listenErrorEvent(e, t, r), this.listenMessageEvent(e, t, r);
  }
  listenExitEvent(e, t, r) {
    e.on("exit", e => {
      this.idleWorkers.delete(r), this.busyWorkers.delete(r), this.deadWorkers.add(r), this.residentWorkers.delete(r), this._log.debug(`worker[${r}] exits with exit code ${e}.`), this.dispatch(this, t.getId());
    });
  }
  listenErrorEvent(e, t, r) {
    e.on("error", e => {
      (null == e ? undefined : e.dece) && this.errorCallback(e.dece), this._log.debug(`worker[${r}] encounters error: ${e.message}.`);
      const s = this.onWorkError(t);
      this.notifyError(s, constant_js_1.PoolConstant.WORK_ERROR, e);
    });
  }
  listenMessageEvent(e, t, r) {
    e.on("message", async e => {
      await this.workerOnMessage(t, e, r);
    });
  }
  async workerOnMessage(e, t, r) {
    var s, o;
    if (e.updateCacheSize(null !== (s = t.cacheSize) && undefined !== s ? s : 0), t.event === constant_js_1.PoolConstant.WORK_ERROR) {
      this._log.debug(`worker[${r}] has one work error.`);
      const s = this.onWorkError(e),
        o = t.returnVal,
        i = new extended_error_info_js_1.ExtendedErrorInfo(o.message, o.extraInfo, o.stack);
      this.notifyError(s, constant_js_1.PoolConstant.WORK_ERROR, i);
    } else if (t.event === constant_js_1.WorkerEvent.CUSTOM_WORKER_REQUEST) {
      const r = tcb_store_js_1.TcbStore.getTCB(t.id).popCustomAction(t.customType);
      await (null == r ? undefined : r(t.param).catch(e => {
        this._log.debug(`action of customType: ${t.customType} throws error: ${e}`);
      })), e.sendMessageToWorker({
        ...t,
        done: true
      });
    } else {
      this._log.debug(`worker[${r}] has one work done.`), tcb_store_js_1.TcbStore.getTCB(t.id) || this._log.debug(`can not get tcb by id ${t.id}`);
      const s = tcb_store_js_1.TcbStore.getTCB(t.id);
      if (this.callbacks.has(t.id) && t.event === constant_js_1.PoolConstant.WORK_DONE) {
        const o = this.callbacks.get(t.id);
        try {
          await (null == o ? undefined : o.fn(...((null == s ? undefined : s.useReturnVal()) ? [t.returnVal] : o.input), r));
        } catch (t) {
          this.notifyError(this.onWorkError(e), constant_js_1.PoolConstant.CALLBACK_ERROR, t);
        }
      }
      e.setLastBusyTime(Date.now()), e.cancelCurWorkId(), t.startTime && (null == s || s.setStartTime(t.startTime)), null == s || s.setEndTime(null !== (o = t.endTime) && undefined !== o ? o : Number(process.hrtime.bigint())), null == s || s.setState(this.eventMap.get(t.event)), this.busyWorkers.delete(e.getId()), e.setState(worker_state_js_1.WorkerState.IDLE), this.idleWorkers.set(e.getId(), e), this.notify(t.id, t.event), this.hvigorMemoryThreshold && this.hvigorMemoryThreshold > 0 ? this.dispatch(this) : this.dispatch(this, e.getId());
    }
  }
  onWorkError(e) {
    var t, r;
    const s = e.getCurWorkId();
    return e.setLastBusyTime(Date.now()), e.cancelCurWorkId(), null === (t = tcb_store_js_1.TcbStore.getTCB(s)) || undefined === t || t.setEndTime(Number(process.hrtime.bigint())), null === (r = tcb_store_js_1.TcbStore.getTCB(s)) || undefined === r || r.setState(this.eventMap.get(constant_js_1.PoolConstant.WORK_ERROR)), this.busyWorkers.delete(e.getId()), e.setState(worker_state_js_1.WorkerState.IDLE), this.idleWorkers.set(e.getId(), e), s;
  }
  _createWorker(e = [], t) {
    return new worker_threads_1.Worker(path_1.resolve(__dirname, constant_js_1.PoolConstant.WORKER_ACTION_PATH), {
      workerData: {
        logLevel: this._log.getLevel().levelStr,
        preludeDeps: e,
        shouldResident: t,
        cacheCapacity: this.cacheCapacity,
        cacheTtl: this.cacheTtl,
        isDaemon: global_core_parameters_js_1.coreParameter.startParams.daemon
      },
      env: process.env,
      stdout: true,
      stderr: true
    });
  }
  getAllocatedId(e) {
    let t = -1;
    if (undefined !== e && (this.isBusyWorker(e) || this.isIdleWorker(e))) return t;
    if (undefined !== e) return t = e, this.isRecycledWorker(e) && this.deadWorkers.delete(t), t;
    if (this.deadWorkers.size) t = this.deadWorkers.values().next().value, this.deadWorkers.delete(t);else for (let e = 0; e < this.maxPoolNum; e++) if (!this.isBusyWorker(e) && !this.isIdleWorker(e)) {
      t = e;
      break;
    }
    return t;
  }
  notify(e, t) {
    const r = tcb_store_js_1.TcbStore.getTCB(e);
    r && !r.isWarmUp() && r.emit(t, e);
  }
  notifyError(e, t, r) {
    const s = tcb_store_js_1.TcbStore.getTCB(e);
    s && !s.isWarmUp() && s.emit(t, {
      id: e,
      error: r
    });
  }
  getDataHandler(e) {
    return t => r => {
      var s, o, i;
      if (!e.getCurWorkId()) return void t.write(r);
      const a = index_js_1.iconv.decode(r, constant_js_1.PoolConstant.ENCODING),
        n = {
          type: log_type_js_1.LogType.WORK,
          time: (0, time_util_js_1.isHvigorLogWithTime)(a) ? a.substring(0, constant_js_1.PoolConstant.TIME_PREFIX_LENGTH) : constant_js_1.PoolConstant.UNKNOWN_LOG_TIME,
          workerId: e.getId(),
          content: a,
          taskPath: null === (s = tcb_store_js_1.TcbStore.getTCB(e.getCurWorkId())) || undefined === s ? undefined : s.getTaskPath(),
          taskName: null === (o = tcb_store_js_1.TcbStore.getTCB(e.getCurWorkId())) || undefined === o ? undefined : o.getTaskName(),
          taskCompletePath: null === (i = tcb_store_js_1.TcbStore.getTCB(e.getCurWorkId())) || undefined === i ? undefined : i.getTaskCompletePath()
        };
      this.addLog(n), t.write(r);
    };
  }
  doWork(e, t, r, s) {
    var o;
    if (!this.idleWorkers.has(s)) return false;
    if (0 !== this.busyWorkers.size && this.hvigorMemoryThreshold && this.hvigorMemoryThreshold > 0 && process.memoryUsage().rss / 1024 / 1024 >= this.hvigorMemoryThreshold && e.isMemorySensitive()) return this._log.debug(`A work dispatched to worker[${null === (o = tcb_store_js_1.TcbStore.getTCB(e.getId())) || undefined === o ? undefined : o.getTaskCompletePath()}] failed \n      because of process.memoryUsage().rss:${process.memoryUsage().rss / 1024 / 1024} > ${this.hvigorMemoryThreshold}.`), false;
    const i = this.idleWorkers.get(s);
    return !(i.isMeltdown() || !this.idleWorkers.get(s).doWork(e)) && (this.idleWorkers.delete(s), this.busyWorkers.set(s, i), e.hasSideEffects() && i.meltdown(), tcb_store_js_1.TcbStore.getTCB(e.getId()).setState(task_state_js_1.TaskState.RUNNING), tcb_store_js_1.TcbStore.getTCB(e.getId()).setStartTime(Number(process.hrtime.bigint())), tcb_store_js_1.TcbStore.getTCB(e.getId()).setWorkerId(s), this.callbacks.set(e.getId(), {
      fn: t,
      input: r
    }), true);
  }
  getAvailableWorkers() {
    return Array.from(this.idleWorkers.keys());
  }
  hasAvailableWorkers() {
    return this.idleWorkers.size > 0;
  }
  recycle() {
    this.idleWorkers.forEach((e, t) => {
      this.idleWorkers.size + this.busyWorkers.size > this.minPoolNum && Date.now() - e.getLastBusyTime() > this.maxIdleTime && !this.residentWorkers.has(t) && (this.idleWorkers.delete(t), this.deadWorkers.add(t), e.terminate(noop_js_1.noop));
    });
  }
  setMaxIdleTime(e) {
    e > 0 && (this.maxIdleTime = e);
  }
  setRecycleInterval(e) {
    e > 0 && (this.recycleInterval = e, clearInterval(this.timer), this.timer = setInterval(() => {
      this.recycle();
    }, this.recycleInterval));
  }
  setErrorCallback(e) {
    this.errorCallback === noop_js_1.noop && (this.errorCallback = e);
  }
  isBusyWorker(e) {
    return this.busyWorkers.has(e);
  }
  isIdleWorker(e) {
    return this.idleWorkers.has(e);
  }
  isRecycledWorker(e) {
    return this.deadWorkers.has(e);
  }
  isFull() {
    return this.idleWorkers.size + this.busyWorkers.size >= this.maxPoolNum;
  }
}
