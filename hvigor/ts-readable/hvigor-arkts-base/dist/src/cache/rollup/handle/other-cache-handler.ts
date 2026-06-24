/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/cache/rollup/handle/other-cache-handler.js
 */
import * as abstract_file_cache_handler_js_1 from "./abstract-file-cache-handler";
import path_1 from "path";
import * as pack_js_1 from "../../../util/msgpack/pack";
import * as fse from "fs-extra";
export class OtherCacheHandler extends abstract_file_cache_handler_js_1.AbstractFileCacheHandler {
  constructor() {
    super(...arguments), this.handlerName = "other";
  }
  handlerReadResult(e, t, r) {
    e[path_1.parse(r).name] = t;
  }
  writeFileCache(e, t, r, a, c, i) {
    if (!e) return;
    const s = new Map(),
      {
        modules: n,
        plugins: u,
        ...l
      } = e;
    Object.keys(l).forEach(e => {
      const t = c?.createSubEvent(`'${e}' pack`).start();
      s.set(e, new pack_js_1.Packr().pack(l[e])), (0, pack_js_1.resetPack)(), t?.stop(), this.filterEvent(c, t);
    }), s.forEach((e, t) => {
      r.push(fse.outputFile(path_1.join(this.getCacheAbsolutePath(), `${t}.msgpack`), e));
    });
  }
}
