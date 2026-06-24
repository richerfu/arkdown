/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/previewer/reuse-watch-worker.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as ohos_logger_js_1 from "../log/ohos-logger";
export class ReuseWatchWorkerManager {
  static async reuseWatchWorker(e, r, a) {
    if (r) return ReuseWatchWorkerManager.logger.debug("Use normal worker. Cannot reuse watch worker"), false;
    if (!ReuseWatchWorkerManager.preWorkerConfig || !hvigor_1.watchWorker.getWorker(ReuseWatchWorkerManager.preWorkerConfig.workerId)) return ReuseWatchWorkerManager.logger.debug("No watchWorker available for preview"), false;
    const o = ReuseWatchWorkerManager.preWorkerConfig.workerId;
    return ReuseWatchWorkerManager.canReuseWorker(e, a) && (await hvigor_1.watchWorker.getLastWatchResult(o)) ? (ReuseWatchWorkerManager.logger.debug("Find watchWorker that can be reused for preview"), hvigor_1.watchWorker.sendNewWatchMsgToProcess(o), true) : (ReuseWatchWorkerManager.logger.debug("Find watchWorker that can not be reused for preview, terminate it"), hvigor_1.watchWorker.terminate(o), ReuseWatchWorkerManager.clearPreWorkerConfig(), false);
  }
  static clearPreWorkerConfig() {
    ReuseWatchWorkerManager.preWorkerConfig = undefined;
  }
  static createWorker(e, r, a) {
    const o = parseInt(a());
    r || isNaN(o) || ReuseWatchWorkerManager.updatePreWorkerConfig({
      ...e,
      workerId: o
    });
  }
  static updatePreWorkerConfig(e) {
    ReuseWatchWorkerManager.preWorkerConfig = e;
  }
  static canReuseWorker(e, r) {
    if (!ReuseWatchWorkerManager.preWorkerConfig) return false;
    const a = ReuseWatchWorkerManager.preWorkerConfig;
    return e.modulePath === a.modulePath && e.pageType === a.pageType && (!r || a.pagesPath.includes(path_1.normalize(r)));
  }
}
ReuseWatchWorkerManager.logger = ohos_logger_js_1.OhosLogger.getLogger(ReuseWatchWorkerManager.name), ReuseWatchWorkerManager.preWorkerConfig = undefined;
