/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/constant/constant.js
 */
import os_1 from "os";
export class PoolConstant {}
export var WorkerEvent;
PoolConstant.MAX_POOL_NUM = os_1.cpus().length - 1, PoolConstant.MIN_POOL_NUM = 0, PoolConstant.DEFAULT_MAX_IDLE_TIME = 6e5, PoolConstant.DEFAULT_RECYCLE_INTERVAL = 6e5, PoolConstant.WORKER_ACTION_PATH = "./worker-action.js", PoolConstant.WORK_DONE = "work done", PoolConstant.WORK_ERROR = "work error", PoolConstant.CALLBACK_ERROR = "callback error", PoolConstant.FILE_SUFFIX = ".js", PoolConstant.UNKNOWN_LOG_TIME = "unknown", PoolConstant.ENCODING = "utf8", PoolConstant.TIME_PREFIX_LENGTH = 25, PoolConstant.CPU_LIMIT = .8, PoolConstant.FAIL_DISPATCH_INTERVAL = 100, PoolConstant.MAX_FAIL_ATTEMPTS = 5, PoolConstant.TSC_COMMON_CACHE_KEY = "common", function (o) {
  o.CLEAN_HVIGOR_CONFIG_LOADER = "CLEAN_HVIGOR_CONFIG_LOADER", o.CLEAN_HVIGOR_LOGGER = "CLEAN_HVIGOR_LOGGER", o.UNMOUNT_TSC_COMMON_CACHE_EVENT = "UNMOUNT_TSC_COMMON_CACHE_EVENT", o.UPDATE_LOG_LEVEL = "UPDATE_LOG_LEVEL", o.CUSTOM_WORKER_REQUEST = "CUSTOM_WORKER_REQUEST";
}(WorkerEvent = WorkerEvent || (WorkerEvent = {}));
