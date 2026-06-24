/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/cache/rollup/rollup-cache-service.js
 */
import path_1 from "path";
import fs_extra_1 from "fs-extra";
import * as arkts_base_log_js_1 from "../../log/arkts-base-log";
import * as constants_js_1 from "../../util/constants";
import * as pack_js_1 from "../../util/msgpack/pack";
import * as struct_js_1 from "../../util/msgpack/struct";
import * as utils_js_1 from "../../util/utils";
import * as cache_handler_factory_js_1 from "./cache-handler-factory";
const logger = arkts_base_log_js_1.ArktsBaseLogger.getLogger("RollupCacheUtil");
(0, struct_js_1.updateReadStruct)();
class RollupCacheService {
  readCache(e, t) {
    if (!e.enableFileSystemCache) return;
    const a = this.getCacheDir(e.cacheDirectory);
    e.cache || (e.cache = {
      modules: []
    }), cache_handler_factory_js_1.CacheHandlerFactory.getHandlerChain(a, t).readCache(e);
  }
  async writeCache({
    enableFileSystemCache: e,
    cache: t,
    cacheDirectory: a,
    pkgName2SourceRoots: c,
    cacheKey: r,
    cacheStoreManager: s,
    useTempFile: o = true,
    parentEvent: l,
    changedModules: n,
    usePathPlaceholder: h = false
  }) {
    if (e && t) {
      const e = this.getCacheDir(a);
      try {
        const a = l?.createSubEvent("get final cache").start(),
          i = this.getFinalCache(t, c);
        if (a?.stop(), i) {
          const t = l?.createSubEvent("pack cache").start(),
            a = [],
            c = new Set(i.modules.map(e => e.id));
          cache_handler_factory_js_1.CacheHandlerFactory.getHandlerChain(e, s).writeCache({
            cache: i,
            allModules: c,
            writePrm: a,
            packCacheEvent: t,
            changedModules: n,
            usePathPlaceholder: h
          }), t?.stop();
          const r = l?.createSubEvent("write cache").start();
          await Promise.all(a), r?.stop();
        }
        s && r && !o && s.mount(r).setCache(e, i, true);
      } catch (e) {
        logger.debug(`write cache file failed ${e.message}`);
      }
      (0, pack_js_1.resetPack)();
    }
  }
  async onBuildEnd(e, t, a, c, r, s) {
    const o = this.getCacheDir(e);
    await cache_handler_factory_js_1.CacheHandlerFactory.getHandlerChain(o, s).onBuildEnd(t, a, c, this.getFinalCache(t.cache, r));
  }
  getCacheDir(e) {
    return e ? path_1.join(e, constants_js_1.MSGPACK_COMPILE_CACHE_DIR_NAME) : constants_js_1.MSGPACK_COMPILE_CACHE_DIR_NAME;
  }
  getFinalCache(e, t) {
    return e && t ? {
      ...e,
      pkgName2SourceRoots: t
    } : e;
  }
  deleteModuleCache(e, t, a, c) {
    if (!arkts_base_log_js_1.ArktsBaseLogger.checkLoggerHaveErrorInfo()) return;
    if (!t) return;
    const r = this.getCacheDir(t);
    cache_handler_factory_js_1.CacheHandlerFactory.getHandlerChain(r, a).deleteModuleCache(e, c);
  }
  deleteCache(e) {
    const t = this.getCacheDir(e);
    fs_extra_1.removeSync(t);
  }
  removeUncacheableModules(e) {
    const t = e.cache;
    if (!t || !t?.modules) return;
    const a = e.startEvent("remove uncacheable modules"),
      c = e.extensions,
      r = e.pkgName2SourceRoots;
    t.modules = t.modules.filter(e => {
      const a = Object.values(e.resolvedIds || {}),
        s = e.meta.pkgName,
        o = r?.[s],
        l = t.pkgName2SourceRoots?.[s];
      return (l || []).join(",") !== (o || []).join(",") ? !a.some(e => !!e.meta?.resolvedBySourceRoot) : a.every(e => {
        const t = e.meta;
        return !t?.resolvedBySourceRoot || !t?.backupSourceRoots?.length || !t.backupSourceRoots.some(e => !!(0, utils_js_1.findSuitableFilePath)(e, c || []));
      });
    }), e.endEvent(a.getId());
  }
}
export const rollupCacheService = new RollupCacheService();
