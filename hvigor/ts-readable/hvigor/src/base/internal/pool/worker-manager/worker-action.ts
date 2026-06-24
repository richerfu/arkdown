/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/worker-manager/worker-action.js
 */
import path_1 from "path";
import * as worker_threads_1 from "worker_threads";
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
import * as log4js_1 from "log4js";
import * as hvigor_config_loader_js_1 from "../../../../common/util/hvigor-config-loader";
import * as default_configuration_js_1 from "../../../log/default-configuration";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
import * as constant_js_1 from "../constant/constant";
import * as cache_store_js_1 from "../store/cache-store";
var _a, _b;
let workerCacheManager;
async function defaultHandler(e) {
  var t;
  const r = handleExitEvent();
  r.onExit();
  const o = handleCustomWorkerRequest(e.id);
  let a;
  o.onCustomWorkerRequest();
  const n = e.workPath,
    s = e.inputData;
  if (n.indexOf(constant_js_1.PoolConstant.FILE_SUFFIX) > 0) {
    const r = n.indexOf(constant_js_1.PoolConstant.FILE_SUFFIX) + constant_js_1.PoolConstant.FILE_SUFFIX.length,
      o = n.slice(0, r),
      s = n.slice(r + 1).split(path_1.sep);
    let _ = await (t = o, Promise.resolve().then(() => __importStar(require(t))));
    if (s.length && "" !== s[0]) {
      for (let t = 0; t < s.length; t++) {
        const r = s[t];
        if ("" === r) {
          if (t === s.length - 1) {
            _ = _.default;
            break;
          }
          return void worker_threads_1.parentPort.postMessage({
            event: constant_js_1.PoolConstant.WORK_ERROR,
            id: e.id
          });
        }
        _ = _[r];
      }
      a = _;
    } else a = _.default;
  }
  const _ = Number(process.hrtime.bigint()),
    i = await a(s, workerCacheManager),
    c = Number(process.hrtime.bigint());
  worker_threads_1.parentPort.postMessage(getReturnMessage(i, e.id, _, c)), o.offCustomWorkerRequest(), r.offExit(), e.exitAfterDone && process.exit(0);
}
function handleExitEvent() {
  const e = e => {
    if (0 !== e) throw new Error(`A process.exit was called in worker thread with exit code ${e}`);
  };
  return {
    onExit: () => {
      process.on("exit", e);
    },
    offExit: () => {
      process.off("exit", e);
    }
  };
}
function handleCustomWorkerRequest(e) {
  const t = t => {
    worker_threads_1.parentPort.postMessage({
      ...t,
      id: e,
      event: constant_js_1.WorkerEvent.CUSTOM_WORKER_REQUEST
    });
  };
  return {
    onCustomWorkerRequest: () => {
      worker_threads_1.parentPort.on(constant_js_1.WorkerEvent.CUSTOM_WORKER_REQUEST, t);
    },
    offCustomWorkerRequest: () => {
      worker_threads_1.parentPort.off(constant_js_1.WorkerEvent.CUSTOM_WORKER_REQUEST, t);
    }
  };
}
function unmountTscCommonCache() {
  null == workerCacheManager || workerCacheManager.unmount(constant_js_1.PoolConstant.TSC_COMMON_CACHE_KEY);
}
function getReturnMessage(e, t, r, o) {
  const a = null == workerCacheManager ? undefined : workerCacheManager.cacheItemSize();
  return e instanceof Error ? {
    event: constant_js_1.PoolConstant.WORK_ERROR,
    id: t,
    returnVal: {
      name: e.name,
      message: e.message,
      stack: e.stack,
      extraInfo: e.extraInfo
    },
    cacheSize: a,
    startTime: r,
    endTime: o
  } : {
    event: constant_js_1.PoolConstant.WORK_DONE,
    id: t,
    returnVal: e,
    cacheSize: a,
    startTime: r,
    endTime: o
  };
}
(0, default_configuration_js_1.setCategoriesLevel)(null !== (_a = default_configuration_js_1.levelMap.get(worker_threads_1.workerData.logLevel)) && undefined !== _a ? _a : log4js_1.levels.INFO), hvigor_logger_1.HvigorLoggerConfig.setCategoriesLevel(null !== (_b = default_configuration_js_1.levelMap.get(worker_threads_1.workerData.logLevel)) && undefined !== _b ? _b : log4js_1.levels.INFO), log4js_1.default.shutdown(), log4js_1.default.configure((0, default_configuration_js_1.getConfiguration)()), worker_threads_1.workerData.shouldResident && (workerCacheManager = new cache_store_js_1.CacheStoreManager({
  capacity: worker_threads_1.workerData.cacheCapacity,
  ttl: worker_threads_1.workerData.cacheTtl
})), worker_threads_1.parentPort.on("message", async e => {
  switch (e.event) {
    case constant_js_1.WorkerEvent.CLEAN_HVIGOR_CONFIG_LOADER:
      hvigor_config_loader_js_1.HvigorConfigLoader.clean();
      break;
    case constant_js_1.WorkerEvent.CLEAN_HVIGOR_LOGGER:
      hvigor_log_js_1.HvigorLogger.clean();
      break;
    case constant_js_1.WorkerEvent.UNMOUNT_TSC_COMMON_CACHE_EVENT:
      unmountTscCommonCache();
      break;
    case constant_js_1.WorkerEvent.UPDATE_LOG_LEVEL:
      (0, hvigor_log_js_1.evaluateLogLevel)(e.logLevel);
      break;
    case constant_js_1.WorkerEvent.CUSTOM_WORKER_REQUEST:
      break;
    default:
      await defaultHandler(e);
  }
});
for (const e of worker_threads_1.workerData.preludeDeps) require(e);
