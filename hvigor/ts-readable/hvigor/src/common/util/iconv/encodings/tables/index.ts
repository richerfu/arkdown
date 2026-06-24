/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/iconv/encodings/tables/index.js
 */
import path_1 from "path";
export const tableInit = function (t) {
  return require(path_1.resolve(__dirname, `../../../../../../res/${t}`));
};
