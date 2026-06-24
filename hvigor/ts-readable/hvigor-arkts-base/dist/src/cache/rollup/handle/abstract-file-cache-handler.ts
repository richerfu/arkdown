/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/cache/rollup/handle/abstract-file-cache-handler.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as fse from "fs-extra";
import * as arkts_base_log_js_1 from "../../../log/arkts-base-log";
import * as pack_js_1 from "../../../util/msgpack/pack";
import * as project_config_path_js_1 from "../../../util/project-config-path";
import * as abstract_cache_handler_js_1 from "./abstract-cache-handler";
const logger = arkts_base_log_js_1.ArktsBaseLogger.getLogger("AbstractFileCacheHandler"),
  FILE_NAME_MAX_LENGTH = 241;
export class AbstractFileCacheHandler extends abstract_cache_handler_js_1.AbstractCacheHandler {
  constructor(e, t) {
    super(e), this.cacheStoreManager = t;
  }
  processCacheDir(e, t) {
    fs_1.readdirSync(e).forEach(a => {
      const r = path_1.join(e, a),
        c = new pack_js_1.Packr().unpack(fs_1.readFileSync(r));
      (0, pack_js_1.resetPack)(), undefined !== t.cache && false !== t.cache && this.handlerReadResult(t.cache, c, r);
    });
  }
  readCache(e) {
    if (this.cacheStoreManager) {
      if (this.cacheStoreManager.mount(e.compileEnv).getCache(path_1.dirname(this.getCacheAbsolutePath()))) return void this.next?.readCache(e);
    }
    const t = this.getCacheAbsolutePath();
    try {
      fse.existsSync(t) && fse.statSync(t).isDirectory() && this.processCacheDir(t, e);
    } catch (e) {
      logger.warn(`read cache file failed ${e.message}`);
    }
    (0, pack_js_1.resetPack)(), this.next?.readCache(e);
  }
  writeCache(e) {
    const {
      cache: t,
      allModules: a,
      writePrm: r,
      packCacheEvent: c,
      changedModules: s,
      usePathPlaceholder: i
    } = e;
    this.writeFileCache(t, a, r, i, c, s), this.next?.writeCache(e);
  }
  getHashName(e) {
    return `${`${path_1.basename(e)}-${(0, hvigor_common_1.hash)(project_config_path_js_1.projectConfigPath.projectTopPath ? path_1.relative(project_config_path_js_1.projectConfigPath.projectTopPath, e) : e)}`.slice(-241)}.msgpack`;
  }
}
