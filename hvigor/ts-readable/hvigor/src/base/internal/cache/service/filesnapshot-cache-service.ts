/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/service/filesnapshot-cache-service.js
 */
import * as fs from "fs";
import path_1 from "path";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
import * as json_reader_js_1 from "../../../util/json-reader";
import * as snapshot_generator_service_js_1 from "../../snapshot/service/snapshot-generator-service";
import * as filesnapshot_cache_entry_js_1 from "../entry/filesnapshot-cache-entry";
import * as default_cache_service_js_1 from "./default-cache-service";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-file-cache-service");
export class FileSnapShotCacheService extends default_cache_service_js_1.DefaultCacheService {
  constructor(e) {
    super(), this._fileSnapShotCachePath = e;
  }
  initialize() {
    var e;
    const t = path_1.resolve(this._fileSnapShotCachePath);
    if (fs.existsSync(t)) try {
      const t = json_reader_js_1.JsonReader.getJsonText(this._fileSnapShotCachePath);
      null === (e = snapshot_generator_service_js_1.SnapshotGeneratorService.getInstance().loadFileSystemSnapshotCacheFromJson(t)) || undefined === e || e.forEach((e, t) => {
        this.set(t, e);
      });
    } catch (e) {
      _log.debug(`Load file cache from ${this._fileSnapShotCachePath} failed.`);
    }
  }
  set(e, t) {
    const r = new filesnapshot_cache_entry_js_1.FileSnapShotCacheEntry(e, t);
    return this.cacheEntries.set(e, r), r;
  }
}
