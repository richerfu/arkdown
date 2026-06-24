/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/iconv/encodings/index.js
 */
import * as dbcs_codec_js_1 from "./dbcs-codec";
import * as internal_js_1 from "./internal";
export const encodings = {
  ...internal_js_1.InternalEncoding,
  ...dbcs_codec_js_1.DBCSEncoding
};
