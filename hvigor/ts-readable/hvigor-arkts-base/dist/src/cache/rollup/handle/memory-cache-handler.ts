/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/cache/rollup/handle/memory-cache-handler.js
 */
import fs_extra_1 from "fs-extra";
import * as project_config_path_js_1 from "../../../util/project-config-path";
import * as abstract_cache_handler_js_1 from "./abstract-cache-handler";
export class MemoryCacheHandler extends abstract_cache_handler_js_1.AbstractCacheHandler {
  constructor(e, t) {
    super(e), this.handlerName = "", this.cacheStoreManager = t;
  }
  readCache(e) {
    if (fs_extra_1.existsSync(this.getCacheAbsolutePath()) && this.cacheStoreManager) {
      const t = this.cacheStoreManager.mount(e.compileEnv).getCache(this.getCacheAbsolutePath());
      t && (t.plugins && project_config_path_js_1.projectConfigPath.replacePluginsCachePath(t.plugins, false), t.modules.forEach(e => {
        e && project_config_path_js_1.projectConfigPath.replaceModuleCachePath(e, false);
      }), e.cache = t);
    }
    this.next?.readCache(e);
  }
  writeCache(e) {
    this.next?.writeCache(e);
  }
  async onBuildEnd(e, t, a, c) {
    a && (await e.close(), t && this.cacheStoreManager?.mount(t).setCache(this.cacheDir, c, true)), await this.next?.onBuildEnd(e, t, a, c);
  }
  deleteModuleCache(e, t) {
    if (this.cacheStoreManager && t) {
      const a = this.cacheStoreManager?.mount(t).getCache(this.getCacheAbsolutePath());
      if (!a?.modules) return;
      for (let t = 0; t < a.modules.length; t++) {
        if (a.modules[t].id === e) {
          a.modules.splice(t, 1);
          break;
        }
      }
    }
    this.next?.deleteModuleCache(e, t);
  }
}
