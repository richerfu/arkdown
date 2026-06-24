/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/service/tasksnapshot-cache-service.js
 */
import * as fs from "fs";
import path_1 from "path";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
import * as json_reader_js_1 from "../../../util/json-reader";
import * as snapshot_generator_service_js_1 from "../../snapshot/service/snapshot-generator-service";
import * as tasksnapshot_cache_entry_js_1 from "../entry/tasksnapshot-cache-entry";
import * as default_cache_service_js_1 from "./default-cache-service";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-task-cache-service");
export class TaskSnapShotCacheService extends default_cache_service_js_1.DefaultCacheService {
  constructor(e) {
    super(), this._taskSnapShotCachePath = e;
  }
  set(e, t) {
    const r = new tasksnapshot_cache_entry_js_1.TaskSnapShotCacheEntry(e, t);
    return this.cacheEntries.set(e, r), r;
  }
  initialize() {
    const e = path_1.resolve(this._taskSnapShotCachePath);
    if (fs.existsSync(e)) try {
      const e = json_reader_js_1.JsonReader.getJsonText(this._taskSnapShotCachePath);
      snapshot_generator_service_js_1.SnapshotGeneratorService.getInstance().loadTaskSnapshotCacheFromJson(e).forEach((e, t) => {
        this.set(t, e);
      });
    } catch (e) {
      _log.debug(`Failed to load the task cache from ${this._taskSnapShotCachePath}.`);
    }
  }
}
