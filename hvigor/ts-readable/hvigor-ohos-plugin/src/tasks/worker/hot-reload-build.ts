/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/hot-reload-build.js
 */
import * as worker_threads_1 from "worker_threads";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as util_js_1 from "./util";
const updatePkgName2SourceRoots = (0, util_js_1.onTerminateWorker)().updatePkgName2SourceRoots;
(0, util_js_1.setLogLevel)(worker_threads_1.workerData.logLevel);
const logger = ohos_logger_js_1.OhosLogger.getLogger("Hot_Reload"),
  projectConfig = worker_threads_1.workerData.projectConfig;
(0, util_js_1.generateLoaderEnv)(projectConfig);
const onBuildFailed = e => {
  var o;
  let r;
  r = e instanceof hvigor_arkts_compose_1.ExtendedErrorInfo ? undefined === e.extraInfo ? e.message : {
    message: e.message,
    warnMsg: null === (o = e.extraInfo) || undefined === o ? undefined : o.warnMsg
  } : e instanceof Error ? e.message : e || "unknown error", worker_threads_1.parentPort.postMessage({
    event: hvigor_1.PoolConstant.WORK_ERROR,
    error: r
  });
};
worker_threads_1.parentPort.on(hvigor_1.WorkerEvent.CUSTOM_WORKER_REQUEST, util_js_1.onCustomWorkerRequest), (0, hvigor_arkts_compose_1.watch)(projectConfig, logger, (e, o) => {
  updatePkgName2SourceRoots((0, hvigor_arkts_compose_1.getPkgName2SourceRootsMap)(projectConfig)), o ? e.length = 0 : worker_threads_1.parentPort.postMessage({
    type: hvigor_1.WatchEvent.WATCH_LOG,
    content: e
  });
}, worker_threads_1.parentPort).then(e => {
  e.isSuccess ? worker_threads_1.parentPort.postMessage({
    event: hvigor_1.PoolConstant.WORK_DONE,
    returnVal: e.compileEvents
  }) : onBuildFailed(e.error);
}).catch(e => {
  onBuildFailed(e);
}).finally(() => {
  worker_threads_1.parentPort.off(hvigor_1.WorkerEvent.CUSTOM_WORKER_REQUEST, util_js_1.onCustomWorkerRequest), logger.debug("Ark compile finished.");
});
