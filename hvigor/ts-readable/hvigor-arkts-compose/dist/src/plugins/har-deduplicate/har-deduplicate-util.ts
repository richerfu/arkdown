/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/har-deduplicate/har-deduplicate-util.js
 */
import fs_1 from "fs";
import os_1 from "os";
import path_1 from "path";
import * as hsp_collect_har_files_js_1 from "./hsp-collect-har-files";
export function isHspDeduplicateRealCompile(e) {
  return !(!e.isHarDeduplicateEnabled || e.isHarDeduplicateArkCompile || "shared" !== e.moduleType);
}
export function isHspCollectFilesCompile(e) {
  return !(!e.isHarDeduplicateEnabled || !e.isHarDeduplicateArkCompile || "shared" !== e.moduleType);
}
export function getHarDuplicateFileSet(e) {
  if (isHspDeduplicateRealCompile(e)) {
    const t = path_1.join(e.intermediateHarDuplicateDirPath, hsp_collect_har_files_js_1.SOURCE_HAR_DUPLICATE_FILES_INFO);
    if (!fs_1.existsSync(t)) return;
    const i = fs_1.readFileSync(t, {
      encoding: "utf-8"
    });
    return new Set(i.trim().split(os_1.EOL).filter(e => e));
  }
}
