/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/prune/task-prune.js
 */
import * as child_process_1 from "child_process";
import fs_1 from "fs";
import path_1 from "path";
import * as path_const_js_1 from "../../../../common/const/path-const";
import * as core_task_js_1 from "../../../external/task/core-task";
import * as default_task_js_1 from "../../../external/task/default-task";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
import * as json_reader_js_1 from "../../../util/json-reader";
const THIRTY_DAYS_MS = 2592e6,
  LOG = hvigor_log_js_1.HvigorLogger.getLogger("prune");
export class PruneTask extends default_task_js_1.DefaultTask {
  constructor(e) {
    super(e, {
      name: "prune",
      group: core_task_js_1.HvigorTaskGroupType.OTHER_TASK_GROUP,
      isEnabled: true,
      sourcePluginId: "hvigor",
      description: "Clean up old hvigor cache files and remove unreferenced packages from store."
    });
  }
  registryAction() {
    return async () => {
      LOG.info("Clear the cache of the hvigor file");
      const e = await deleteCacheFiles();
      await Promise.all(e), LOG.info("Remove unreferenced packages from the store"), execStorePruneCommand();
    };
  }
}
async function deleteCacheFiles() {
  if (!fs_1.existsSync(path_const_js_1.HVIGOR_PROJECT_CACHES_HOME)) return [];
  return fs_1.readdirSync(path_const_js_1.HVIGOR_PROJECT_CACHES_HOME).map(async e => {
    const t = path_1.resolve(path_const_js_1.HVIGOR_PROJECT_CACHES_HOME, e);
    if (fs_1.statSync(t).isDirectory()) {
      const e = path_1.resolve(t, "workspace", "metadata.json");
      fs_1.existsSync(e) && fs_1.statSync(e).isFile() && needDeleteDir(e) && (LOG.info("Clear the hvigor cache file: ", t), await fs_1.promises.rm(t, {
        force: true,
        recursive: true
      }));
    }
  });
}
function execStorePruneCommand() {
  var e;
  const t = {
      cwd: path_const_js_1.HVIGOR_WRAPPER_TOOLS_HOME,
      stdio: ["inherit", "inherit", "inherit"],
      windowsHide: true
    },
    r = (0, child_process_1.spawnSync)(path_const_js_1.HVIGOR_WRAPPER_PNPM_SCRIPT_PATH, ["store", "prune"], t);
  0 !== r.status && LOG.printErrorExit("PNPM_STORE_PRUNE_EXECUTE_FAILED", [null === (e = r.error) || undefined === e ? undefined : e.message]);
}
function needDeleteDir(e) {
  const t = json_reader_js_1.JsonReader.getJsonObj(e).lastUpdatedTime;
  return !!t && new Date(new Date().getTime() - 2592e6).getTime() > t;
}
