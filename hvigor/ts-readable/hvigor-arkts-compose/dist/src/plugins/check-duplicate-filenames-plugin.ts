/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/check-duplicate-filenames-plugin.js
 */
import path_1 from "path";
import * as hvigor_arkts_base_1 from "@ohos/hvigor-arkts-base";
const fileNamesMap = new Map(),
  duplicatedFileNamesSet = new Set(),
  LOGGER = hvigor_arkts_base_1.ArktsBaseLogger.getPluginLogger("Duplicated filenames check");
export function checkDuplicatedFileNamesPlugin() {
  return {
    name: "checkDuplicatedFileNamesPlugin",
    load: load,
    beforeBuildEnd: beforeBuildEnd,
    cleanUp() {
      fileNamesMap.clear(), duplicatedFileNamesSet.clear();
    }
  };
}
function load(e) {
  const a = path_1.extname(e),
    i = e.slice(0, -a.length);
  fileNamesMap.has(i) ? (duplicatedFileNamesSet.add(fileNamesMap.get(i)), duplicatedFileNamesSet.add(e)) : fileNamesMap.set(i, e);
}
function beforeBuildEnd() {
  duplicatedFileNamesSet.size > 0 && LOGGER.warn(`Duplicate file names detected, which may cause compilation errors. Please check the following files: \n${[...duplicatedFileNamesSet].join("\n")}`), fileNamesMap.clear(), duplicatedFileNamesSet.clear();
}
