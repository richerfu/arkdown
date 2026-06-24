/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/build-steps/rm-error-files-cache.js
 */
import * as arkts_base_log_js_1 from "../log/arkts-base-log";
import * as rollup_cache_service_js_1 from "../cache/rollup/rollup-cache-service";
export async function removeCompileFailedFilesFromCache(e, r, o, s, l) {
  if (!l || !l.modules?.length || !arkts_base_log_js_1.ArktsBaseLogger.checkLoggerHaveErrorInfo()) return;
  const a = e.startEvent("remove compile failed files from cache");
  arkts_base_log_js_1.ArktsBaseLogger.getErrorFileIds().forEach(e => {
    rollup_cache_service_js_1.rollupCacheService.deleteModuleCache(e, r, o, s);
  }), e.endEvent(a.getId());
}
