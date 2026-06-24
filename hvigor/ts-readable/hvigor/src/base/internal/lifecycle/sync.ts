/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/sync.js
 */
import * as path_1 from "path";
import * as local_file_writer_js_1 from "../../../common/util/local-file-writer";
import * as path_util_js_1 from "../../../common/util/path-util";
import * as default_tooling_model_bean_registry_js_1 from "../../external/default-tooling-model-bean-registry";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as default_sync_task_js_1 from "../task/core/default-sync-task";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-process");
export function findSyncTaskPathsForNode(t) {
  return t.getAllTasks().filter(t => t instanceof default_sync_task_js_1.DefaultSyncTask).map(t => t.getPath());
}
export function outputPluginSyncInfo() {
  const t = Object.create(null),
    e = Object.create(null);
  t.version = 1, default_tooling_model_bean_registry_js_1.defaultModelRegistry.getModelMap().forEach((o, r) => {
    t[r] = o.buildModel(), e[r] = o.taskInfo ? o.taskInfo() : [];
  });
  const o = (0, path_1.resolve)(path_util_js_1.PathUtil.getHvigorCacheDir(_log), "outputs", "sync", "output.json"),
    r = (0, path_1.resolve)(path_util_js_1.PathUtil.getHvigorCacheDir(_log), "outputs", "sync", "taskInfo.json");
  try {
    local_file_writer_js_1.LocalFileWriter.getInstance().write(o, t), local_file_writer_js_1.LocalFileWriter.getInstance().write(r, e);
  } catch (t) {
    _log.error("Sync info serialization failed.");
  }
}
