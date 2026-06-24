/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/cxx/hvigor-native-build.js
 */
import fs_extra_1 from "fs-extra";
export const HvigorNativeConfigFile = "hvigor_native_config.json";
export const readHvigorNativeConfigFile = async e => {
  if (fs_extra_1.existsSync(e)) return fs_extra_1.readJSON(e);
};
