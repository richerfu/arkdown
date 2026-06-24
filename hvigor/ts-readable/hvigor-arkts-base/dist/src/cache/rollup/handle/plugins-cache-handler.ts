/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/cache/rollup/handle/plugins-cache-handler.js
 */
import * as abstract_file_cache_handler_js_1 from "./abstract-file-cache-handler";
import * as project_config_path_js_1 from "../../../util/project-config-path";
import * as pack_js_1 from "../../../util/msgpack/pack";
import * as fse from "fs-extra";
import path_1 from "path";
export class PluginsCacheHandler extends abstract_file_cache_handler_js_1.AbstractFileCacheHandler {
  constructor() {
    super(...arguments), this.handlerName = "plugins";
  }
  handlerReadResult(e, t) {
    project_config_path_js_1.projectConfigPath.replacePluginsCachePath(t, false), e.plugins = t;
  }
  writeFileCache(e, t, r, a, i, n) {
    if (!e || !e.plugins) return;
    const c = i?.createSubEvent("plugin pack").start();
    a && project_config_path_js_1.projectConfigPath.replacePluginsCachePath(e.plugins, true);
    const s = new pack_js_1.Packr().pack(e.plugins);
    (0, pack_js_1.resetPack)(), c?.stop(), this.filterEvent(i, c), r.push(fse.outputFile(path_1.join(this.getCacheAbsolutePath(), "plugins.msgpack"), s));
  }
}
