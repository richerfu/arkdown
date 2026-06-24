/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/cache-libs.js
 */
import * as libs_file_cache_util_js_1 from "../../utils/libs-file-cache-util";
export const cacheNativeLibs = async e => {
  await libs_file_cache_util_js_1.LibsFileCacheUtil.refreshBinxoFileCache(e.intermediatesProcessLibs, e.binxoNativeLibs, e.cacheBinxoFilePath, e.binxoSymbol), await libs_file_cache_util_js_1.LibsFileCacheUtil.refreshLibsFileCache(e.inputCacheStripLibs, e.strippedNativeLibs, e.cacheStripFilePath, e.debugSymbol);
};
