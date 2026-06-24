/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/build-steps/clean-ets-for-tgz.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as fse from "fs-extra";
import * as rollup_cache_service_js_1 from "../cache/rollup/rollup-cache-service";
import * as constants_js_1 from "../util/constants";
export function cleanEtsForTgz(e, t) {
  if (!e.declaredFilesPath || !fse.existsSync(e.declaredFilesPath)) return;
  const r = rollup_cache_service_js_1.rollupCacheService.getCacheDir(e.cacheDirectory),
    i = path_1.resolve(e.arkCompileCachePath, constants_js_1.COMPILE_INFO);
  try {
    if (fs_1.existsSync(r) && fs_1.existsSync(i)) {
      fse.readJSONSync(i).buildMode !== e.buildMode && fse.removeSync(e.declaredFilesPath);
    } else fse.removeSync(e.declaredFilesPath);
  } catch (r) {
    t.printErrorExit("DELETE_DIRECTORY_ERROR", [e.declaredFilesPath]);
  }
}
