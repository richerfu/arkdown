/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/preview-build.js
 */
import * as worker_threads_1 from "worker_threads";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as util_js_1 from "./util";
const updatePkgName2SourceRoots = (0, util_js_1.onTerminateWorker)().updatePkgName2SourceRoots;
(0, util_js_1.setLogLevel)(worker_threads_1.workerData.logLevel);
const logger = ohos_logger_js_1.OhosLogger.getLogger("Preview_Build"),
  projectConfig = worker_threads_1.workerData.projectConfig;
(0, util_js_1.generateLoaderEnv)(projectConfig);
const onBuildFailed = e => {
  const r = e instanceof Error ? e.message : e || "unknown error";
  worker_threads_1.parentPort.postMessage({
    event: hvigor_1.PoolConstant.WORK_ERROR,
    error: r
  });
};
function parseFileId(e) {
  if ("string" != typeof e.msg) return false;
  const r = "[33mArkTS:WARN File: ";
  if (e.msg.startsWith(r)) return t(e.msg.substring(22));
  const o = "[31mArkTS:ERROR File: ";
  return !!e.msg.startsWith(o) && t(e.msg.substring(23));
  function t(r) {
    let o = r.split("\n")[0];
    for (let e = 0; e < 2; e++) {
      const e = o.lastIndexOf(":");
      if (-1 === e) return false;
      o = o.substring(0, e);
    }
    return e.fileId = o, true;
  }
}
worker_threads_1.parentPort.on(hvigor_1.WorkerEvent.CUSTOM_WORKER_REQUEST, util_js_1.onCustomWorkerRequest), (0, hvigor_arkts_compose_1.watch)(projectConfig, logger, (e, r) => {
  if (updatePkgName2SourceRoots((0, hvigor_arkts_compose_1.getPkgName2SourceRootsMap)(projectConfig)), r) {
    const o = [];
    for (const t of e) parseFileId(t) && t.fileId && r.has(t.fileId) && (o.push(t), worker_threads_1.parentPort.postMessage({
      type: hvigor_1.WatchEvent.WATCH_LOG,
      content: [t]
    }));
    e.length = 0, e.push(...o);
  } else worker_threads_1.parentPort.postMessage({
    type: hvigor_1.WatchEvent.WATCH_LOG,
    content: e
  });
}, worker_threads_1.parentPort).then(e => {
  e.isSuccess ? (worker_threads_1.parentPort.on("message", async e => {
    e && e.command && e.data && e.data.script ? (0, hvigor_arkts_compose_1.compileCodeSnippet)(e) : logger.debug(`bad codeSnippet ${e}`);
  }), worker_threads_1.parentPort.postMessage({
    event: hvigor_1.PoolConstant.WORK_DONE,
    returnVal: e.compileEvents
  })) : onBuildFailed(e.error);
}).catch(e => {
  onBuildFailed(e);
}).finally(() => {
  worker_threads_1.parentPort.off(hvigor_1.WorkerEvent.CUSTOM_WORKER_REQUEST, util_js_1.onCustomWorkerRequest), logger.debug("Ark compile finished.");
});
