/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/gc-plugin.js
 */
import * as fs_js_1 from "../util/fs";
const gcFilePrefix = ".js";
let hasShareCache = false;
export function gcPlugin() {
  return {
    name: "gcPlugin",
    beforeBuild: gcBeforeBuild,
    transform: gcPluginTransform,
    afterBuildStart: gcHandler()
  };
}
function gcHandler() {
  return {
    order: "post",
    handler() {
      hasShareCache || global?.gc?.();
    }
  };
}
function gcBeforeBuild() {
  const e = this.share?.cache?.size();
  hasShareCache = undefined !== e && e > 0;
}
async function gcPluginTransform(e, r) {
  r.endsWith(".js") && (await (0, fs_js_1.checkFileExistsAndSize)(r)) && global.gc && global.gc();
}
