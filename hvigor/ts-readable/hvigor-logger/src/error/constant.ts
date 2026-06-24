/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/constant.js
 */
import path_1 from "path";
import * as type_js_1 from "./type";
export const ERROR_INFO_DIRCTORY_PATH = path_1.resolve(__dirname, "../../res/error");
export const ErrorCodeToFile = {
  [type_js_1.ToolErrorCode.TEC_00]: "global.json",
  [type_js_1.ToolErrorCode.TEC_10]: "hvigor.json",
  [type_js_1.ToolErrorCode.TEC_11]: "hvigor-ohos-plugin.json",
  [type_js_1.ToolErrorCode.TEC_12]: "hvigor-compiler.json",
  [type_js_1.ToolErrorCode.TEC_21]: "ark-compiler-toolchain.json",
  [type_js_1.ToolErrorCode.TEC_22]: "pack-tool.json",
  [type_js_1.ToolErrorCode.TEC_23]: "restool.json",
  [type_js_1.ToolErrorCode.TEC_24]: "sign-tool.json"
};
export const DEFAULT_ERROR_CODE = "00000000";
export const ERROR_NOT_MATCH = "The error information does not match.";
export const ERROR_MESSAGE_HIDDEN = "Error message is hidden for security reasons";
export const MATCH_FIELD_TYPES = ["none", "id", "checkMessage", "code"];
export const EIGHT_DIGIT_ERROR_REGEXP = {
  PATTERN: "ERROR.{0,12}(?<code>\\d{8})(?!\\d)",
  FLAGS: "gis"
};
