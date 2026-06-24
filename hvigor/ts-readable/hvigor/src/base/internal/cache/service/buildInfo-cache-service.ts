/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/service/buildInfo-cache-service.js
 */
import * as fs from "fs";
import path_1 from "path";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
import * as json_reader_js_1 from "../../../util/json-reader";
import * as snapshot_generator_service_js_1 from "../../snapshot/service/snapshot-generator-service";
import * as default_cache_service_js_1 from "./default-cache-service";
import * as buildInfo_cache_entry_js_1 from "../entry/buildInfo-cache-entry";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-buildInfo-cache-service");
export class BuildInfoCacheService extends default_cache_service_js_1.DefaultCacheService {
  constructor(e) {
    super(), this._buildInfoCachePath = e;
  }
  set(e, t) {
    const r = new buildInfo_cache_entry_js_1.BuildInfoCacheEntry(e, t);
    return this.cacheEntries.set(e, r), r;
  }
  initialize() {
    const e = path_1.resolve(this._buildInfoCachePath);
    if (fs.existsSync(e)) try {
      const e = json_reader_js_1.JsonReader.getJsonText(this._buildInfoCachePath);
      snapshot_generator_service_js_1.SnapshotGeneratorService.getInstance().loadBuildInfoSnapshotCacheFromJson(e).forEach((e, t) => {
        this.set(t, e);
      });
    } catch (e) {
      _log.debug(`Failed to load the build info cache from ${this._buildInfoCachePath}.`);
    }
  }
}
