/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/cache/rollup/handle/modules-cache-handler.js
 */
import path_1 from "path";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as arkts_base_log_js_1 from "../../../log/arkts-base-log";
import * as pack_js_1 from "../../../util/msgpack/pack";
import * as project_config_path_js_1 from "../../../util/project-config-path";
import * as abstract_file_cache_handler_js_1 from "./abstract-file-cache-handler";
const logger = arkts_base_log_js_1.ArktsBaseLogger.getLogger("ModulesCacheHandler"),
  MAX_CONCURRENT_WRITE = 6e3;
export class ModulesCacheHandler extends abstract_file_cache_handler_js_1.AbstractFileCacheHandler {
  constructor() {
    super(...arguments), this.handlerName = "modules";
  }
  handlerReadResult(e, t) {
    project_config_path_js_1.projectConfigPath.replaceModuleCachePath(t, false), e.modules.push(t);
  }
  async onBuildEnd(e, t, a, s) {
    if (!fs_extra_1.existsSync(this.getCacheAbsolutePath())) return;
    const r = await fs_extra_1.readdir(this.getCacheAbsolutePath());
    for (const e of r) e.startsWith("new-") && (a ? await fs_extra_1.rename(path_1.resolve(this.getCacheAbsolutePath(), e), path_1.resolve(this.getCacheAbsolutePath(), e.replace("new-", ""))) : await fs_extra_1.rm(path_1.resolve(this.getCacheAbsolutePath(), e)));
    await this.next?.onBuildEnd(e, t, a, s);
  }
  deleteModuleCache(e, t) {
    const a = this.getHashName(e),
      s = path_1.join(this.getCacheAbsolutePath(), a);
    fs_extra_1.existsSync(s) && fs_extra_1.removeSync(s), this.next?.deleteModuleCache(e, t);
  }
  writeFileCache(e, t, a, s, r, o) {
    if (!e) return;
    const h = new Map();
    e.modules.forEach(e => {
      const t = e.id;
      if (t.startsWith("\0")) logger.debug(`Virtual file: ${t} is not written to cache.`);else if (undefined === o || o.has(t)) {
        s && project_config_path_js_1.projectConfigPath.replaceModuleCachePath(e, true);
        const a = r?.createSubEvent(`module '${path_1.basename(t)}' pack`).start();
        h.set(t, new pack_js_1.Packr().pack(e)), (0, pack_js_1.resetPack)(), a?.stop(), this.filterEvent(r, a);
      }
    });
    const l = new Map(),
      _ = this.getCacheAbsolutePath(),
      i = (0, hvigor_common_1.createPromiseLimit)(6e3);
    h.forEach((e, t) => {
      const s = () => fs_extra_1.outputFile(path_1.join(this.getCacheAbsolutePath(), `new-${this.getHashName(t)}`), e);
      if ((0, hvigor_common_1.isWindows)()) a.push(i(s));else {
        const e = Promise.resolve();
        a.push(e.then(s));
      }
    }), fs_extra_1.existsSync(_) && fs_extra_1.readdirSync(_).forEach(e => {
      l.set(e, path_1.join(_, e));
    }), t.forEach(e => {
      l.has(this.getHashName(e)) && l.delete(this.getHashName(e));
    }), l.forEach(e => {
      (0, hvigor_common_1.isWindows)() ? a.push(i(() => fs_extra_1.remove(e))) : a.push(fs_extra_1.remove(e));
    });
  }
}
