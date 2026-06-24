/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/cache/rollup/cache-handler-factory.js
 */
import * as memory_cache_handler_js_1 from "./handle/memory-cache-handler";
import * as modules_cache_handler_js_1 from "./handle/modules-cache-handler";
import * as plugins_cache_handler_js_1 from "./handle/plugins-cache-handler";
import * as other_cache_handler_js_1 from "./handle/other-cache-handler";
export class CacheHandlerFactory {
  static getHandlerChain(e, a) {
    const r = new memory_cache_handler_js_1.MemoryCacheHandler(e, a),
      c = new modules_cache_handler_js_1.ModulesCacheHandler(e, a),
      h = new plugins_cache_handler_js_1.PluginsCacheHandler(e, a),
      n = new other_cache_handler_js_1.OtherCacheHandler(e, a);
    return r.next = c, c.next = h, h.next = n, r;
  }
}
