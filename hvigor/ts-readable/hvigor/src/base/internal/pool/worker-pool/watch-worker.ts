/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/worker-pool/watch-worker.js
 */
import cluster_1 from "cluster";
import * as worker_threads_1 from "worker_threads";
import * as const_js_1 from "../../../../common/const/const";
import * as hook_const_js_1 from "../../../../common/const/hook-const";
import * as noop_js_1 from "../../../../common/util/noop";
import * as build_event_js_1 from "../../../common/daemon-protocol/build-event";
import * as worker_process_event_id_js_1 from "../../../daemon/cluster/worker-process-event-id";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
import * as duration_event_js_1 from "../../../metrics/event/duration-event";
import * as log_event_js_1 from "../../../metrics/event/log-event";
import * as hvigor_lifecycle_hook_js_1 from "../../lifecycle/hook/hvigor-lifecycle-hook";
import * as constant_js_1 from "../constant/constant";
import * as extended_error_info_js_1 from "../worker-manager/extended-error-info";
const log = hvigor_log_js_1.HvigorLogger.getLogger("daemon");
class WorkerItem {
  constructor(e, r, t, o) {
    this.lastWatchResult = {
      finish: true,
      result: true
    }, this.resultResolveSet = new Set(), this.customActions = new Map(), this.workerBelongTaskName = t, this.worker = e, this.hasCustomTerminate = r, this.keepAlive = false, o && this.customActions.set(o.customType, o.action);
  }
}
const genPromiseAndTriggers = (e, r) => {
  let t = noop_js_1.noop,
    o = noop_js_1.noop;
  const s = new Promise((s, n) => {
    t = t => {
      e.taskExecutedStatus.setWorkerTimePeriod([r, Number(process.hrtime.bigint())]), n(t);
    }, o = t => {
      e.taskExecutedStatus.setWorkerTimePeriod([r, Number(process.hrtime.bigint())]), s(t);
    };
  });
  return {
    reject: t,
    resolve: o,
    promise: s
  };
};
async function handleResponse(e, r, t, o, s, n) {
  if (e.event !== constant_js_1.PoolConstant.WORK_DONE) {
    if (e.event === constant_js_1.PoolConstant.WORK_ERROR) {
      null == n || n.stop(duration_event_js_1.DurationEventState.FAILED), null == n || n.setLog(null == n ? undefined : n.getName(), log_event_js_1.MetricLogType.ERROR), await s(e.returnVal);
      const a = e.error,
        i = `${r.getName()} ${o ? "watch" : "normal"} work[${t}] failed.`;
      if (log.debug(i), undefined !== a.warnMsg) throw new extended_error_info_js_1.ExtendedErrorInfo(a.message, {
        warnMsg: a.warnMsg
      });
      throw new Error("string" == typeof a ? a : a.message || i);
    }
    throw new Error(`Unknown worker event: ${e}.`);
  }
  null == n || n.stop(duration_event_js_1.DurationEventState.SUCCESS), null == n || n.setLog(null == n ? undefined : n.getName(), log_event_js_1.MetricLogType.INFO), await s(e.returnVal);
}
class WorkerCountManager {
  static allocateWorkerId() {
    return this.cnt++;
  }
  static reset() {
    this.cnt = 1;
  }
}
WorkerCountManager.cnt = 1;
class WatchWorker {
  constructor() {
    this.workerMap = new Map(), this.addListenersOnSessionManager();
  }
  getWorker(e) {
    var r;
    return null === (r = this.workerMap.get(e)) || undefined === r ? undefined : r.worker;
  }
  async getLastWatchResult(e) {
    const r = this.workerMap.get(e);
    return !r || (r.lastWatchResult.finish ? r.lastWatchResult.result : new Promise(e => {
      r.resultResolveSet.add(e);
    }));
  }
  sendCompileMsgToWorker() {
    this.workerMap.forEach((e, r) => {
      e.workerBelongTaskName.includes("HotReload") && e.worker.postMessage({
        event: "HOT_COMPILE"
      });
    });
  }
  sendNewWatchMsgToProcess(e) {
    var r;
    cluster_1.isWorker && process.send({
      type: build_event_js_1.WatchEvent.NEW_WATCH_WORKER,
      workerId: e
    }), null === (r = this.workerMap.get(e)) || undefined === r || r.worker.postMessage({
      event: "PAUSE_PRINT_ERR",
      data: false
    });
  }
  createWorker(e, r, t, o, s, n) {
    const a = e.getName(),
      i = Number(process.hrtime.bigint()),
      l = new worker_threads_1.Worker(r, s),
      c = WorkerCountManager.allocateWorkerId();
    this.workerMap.set(c, new WorkerItem(l, t, a, n));
    const _ = e.addSubDurationEvent(c.toString()).start();
    null == _ || _.setTid(`Worker${c}`), this.sendNewWatchMsgToProcess(c);
    const {
      promise: u,
      resolve: d,
      reject: k
    } = genPromiseAndTriggers(e, i);
    return e.pendingPromises.add(u), this.createWorkerOnMessageListener(l, d, k, e, c, o, _), this.createWorkerOnErrorListener(l, k, e, c), n && this.createCustomActionListener(l, c), c.toString();
  }
  createCustomActionListener(e, r) {
    e.on("message", async t => {
      if (t.event !== const_js_1.CUSTOM_WORKER_REQUEST) return;
      const o = this.workerMap.get(r);
      if (!o) return;
      const s = o.customActions.get(t.customType);
      s && (await s(t.param).catch(e => {
        log.debug(`action of customType: ${t.customType} throws error: ${e}`);
      }), e.postMessage({
        ...t,
        done: true
      }));
    });
  }
  createWorkerOnMessageListener(e, r, t, o, s, n, a) {
    e.on("message", async e => {
      if (e) {
        if (e.event !== constant_js_1.WorkerEvent.CUSTOM_WORKER_REQUEST) if (e.event) try {
          await handleResponse(e, o, s, true, n, a), r("succeed");
        } catch (e) {
          t(e);
        } else {
          if (e.type === build_event_js_1.WatchEvent.WATCH_COMPILE_RESULT || e.type === build_event_js_1.WatchEvent.WATCH_START || e.type === build_event_js_1.WatchEvent.WATCH_RESULT || e.type === build_event_js_1.WatchEvent.WATCH_LOG) {
            log.debug(`watch worker: send response to session manager. Response type: ${e.type}`);
            try {
              await hvigor_lifecycle_hook_js_1.HvigorLifecycleHook.getInstance().runHook(hook_const_js_1.HookType.onWatchWorkerMessage, e, a);
            } catch (e) {
              log.error(`custom onWatchWorkerMessage hook exception: ${e.message}`);
            }
            return this.forwardMessageToProcess(s, e), void this.handleWatchStartAndResult(s, e);
          }
          if (e.type === build_event_js_1.WatchEvent.TERMINATE_WORKER) return log.debug("watch worker: worker is ready to be terminated."), void this.terminateWorker(e.content);
          t(new Error(`Unknown watch event: ${e}.`));
        }
      } else t(new Error("Undefined response."));
    });
  }
  forwardMessageToProcess(e, r) {
    cluster_1.isWorker && ("hotCompile" === r.hotCompile ? process.send({
      type: `${true === r.content ? worker_process_event_id_js_1.WORKER_PROCESS_EVENT_ID.FINISHED : worker_process_event_id_js_1.WORKER_PROCESS_EVENT_ID.FAILED}`
    }) : process.send({
      ...r,
      workerId: e
    }));
  }
  handleWatchStartAndResult(e, r) {
    const t = this.workerMap.get(e);
    if (r.type === build_event_js_1.WatchEvent.WATCH_START && t && (t.lastWatchResult.finish = false), r.type === build_event_js_1.WatchEvent.WATCH_RESULT && t) {
      for (const e of t.resultResolveSet) e(r.content);
      t.resultResolveSet.clear(), t.lastWatchResult = {
        finish: true,
        result: r.content
      };
    }
  }
  createWorkerOnErrorListener(e, r, t, o) {
    e.on("error", async e => {
      log.error(`${t.getName()} watch work[${o}] failed.`), log.error(e), r(e);
    });
  }
  terminate(e) {
    var r, t;
    (null === (r = this.workerMap.get(e)) || undefined === r ? undefined : r.hasCustomTerminate) ? null === (t = this.workerMap.get(e)) || undefined === t || t.worker.postMessage({
      type: build_event_js_1.WatchEvent.TERMINATE_WORKER,
      content: e
    }) : this.terminateWorker(e);
  }
  setAliveForWorker(e) {
    const r = this.workerMap.get(e);
    r && (r.keepAlive = true);
  }
  consumeKeepAlive(e) {
    const r = this.workerMap.get(e);
    return !!(null == r ? undefined : r.keepAlive) && (r.keepAlive = false, true);
  }
  terminateWorker(e) {
    var r;
    null === (r = this.workerMap.get(e)) || undefined === r || r.worker.terminate(), this.workerMap.delete(e), log.debug(`Server currently has ${this.workerMap.size} ${this.workerMap.size > 1 ? "watch-workers" : "watch-worker"}`);
  }
  addListenersOnSessionManager() {
    cluster_1.isWorker && cluster_1.worker.on("message", e => {
      var r, t, o, s;
      if (undefined !== e.workerId && 0 !== e.workerId) {
        if (e.type === build_event_js_1.WatchEvent.WATCH_COMPILE_DATA) log.debug("watch worker: receive watch compile tata."), null === (r = this.getWorker(e.workerId)) || undefined === r || r.postMessage(e.content);else if (e.type === build_event_js_1.WatchEvent.SEND_KEEP_ALIVE_SIGNAL) this.setAliveForWorker(e.workerId);else if (e.type === build_event_js_1.WatchEvent.CLOSE_WATCH) {
          if (this.consumeKeepAlive(e.workerId)) return void (null === (s = null === (o = null === (t = this.workerMap.get(e.workerId)) || undefined === t ? undefined : t.worker) || undefined === o ? undefined : o.postMessage) || undefined === s || s.call(o, {
            event: "PAUSE_PRINT_ERR",
            data: true
          }));
          this.terminate(e.workerId);
        }
      } else log.debug("watch worker: worker id should be larger than 0. Nothing will be sent to any worker thread.");
    });
  }
}
class NormalWorker {
  constructor() {
    this.maxWorkerNum = constant_js_1.PoolConstant.MAX_POOL_NUM > 1 ? constant_js_1.PoolConstant.MAX_POOL_NUM - 1 : 1;
  }
  createWorker(e, r, t, o, s) {
    const n = Number(process.hrtime.bigint()),
      a = WorkerCountManager.allocateWorkerId(),
      {
        promise: i,
        resolve: l,
        reject: c
      } = genPromiseAndTriggers(e, n);
    return e.pendingPromises.add(i), NormalWorker.promiseMap.set(a, {
      resolve: l,
      reject: c
    }), NormalWorker.runningThreadCnt >= this.maxWorkerNum ? (log.debug(`Creating worker failed since runningThreadCnt is not smaller than ${this.maxWorkerNum}.`), NormalWorker.workQueue.push({
      workerId: a,
      task: e,
      filename: r,
      callback: o,
      options: s
    })) : (log.debug(`Creating worker with id ${a}.`), this.executeWorker(a, e, r, o, s)), a.toString();
  }
  reset() {
    NormalWorker.runningThreadCnt = 0, NormalWorker.workQueue.length = 0, NormalWorker.promiseMap.clear();
  }
  executeWorker(e, r, t, o, s) {
    const n = new worker_threads_1.Worker(t, s);
    NormalWorker.runningThreadCnt++;
    const {
      resolve: a,
      reject: i
    } = NormalWorker.promiseMap.get(e);
    NormalWorker.promiseMap.delete(e);
    const l = r.addSubDurationEvent(e.toString()).start();
    null == l || l.setTid(`Worker${e}`), n.on("message", async t => {
      if (t) {
        if (t.event) try {
          await handleResponse(t, r, e, false, o, l), n.terminate(), NormalWorker.runningThreadCnt--, this.processNextTask(), a(true);
        } catch (e) {
          i(e);
        } else t.type !== build_event_js_1.WatchEvent.WATCH_COMPILE_RESULT && t.type !== build_event_js_1.WatchEvent.WATCH_START && t.type !== build_event_js_1.WatchEvent.WATCH_RESULT && t.type !== build_event_js_1.WatchEvent.WATCH_LOG && (t.type !== build_event_js_1.WatchEvent.TERMINATE_WORKER ? i(new Error(`Unknown watch event: ${t}.`)) : log.debug("Impossible response type in normal worker: TERMINATE_WORKER"));
      } else i(new Error("Undefined response."));
    }), n.on("error", async t => {
      log.debug(`${r.getName()} normal work[${e}] failed.`), i(t);
    });
  }
  processNextTask() {
    if (NormalWorker.workQueue.length) {
      const e = NormalWorker.workQueue.shift();
      log.debug(`Normal worker queue is not empty. Create worker with id: ${e.workerId}.`), this.executeWorker(e.workerId, e.task, e.filename, e.callback, e.options);
    }
  }
}
export const watchWorker = new WatchWorker();
export const normalWorker = new NormalWorker();
NormalWorker.runningThreadCnt = 0, NormalWorker.workQueue = [], NormalWorker.promiseMap = new Map();
