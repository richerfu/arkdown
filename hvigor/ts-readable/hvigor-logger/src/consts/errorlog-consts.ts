/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/consts/errorlog-consts.js
 */
import * as url_utils_js_1 from "../utils/url-utils";
export var CountryEnum, MergeType, MergeKeyEnum, SubSystemEnum, LogLevel, MetricLogType, ErrorCode;
export const UNDEFINED_CODE = "00000000";
export const UNDEFINED_DESC = "";
export const UNDEFINED_CAUSE = "Unknown";
export const UNDEFINED_POS = "";
export const BUILD_CACHE_DIR = "build-cache-dir";
export const HVIGOR_USER_HOME_DIR_NAME = ".hvigor";
export const PLUGIN_LOG_TYPE = "plugin_log";
export const NORMAL_LOG_TYPE = "normal_log";
export const SPLIT_TAG = "<HVIGOR_ERROR_SPLIT>";
export const DEFAULT_MORE_INFO_URL_CN = (0, url_utils_js_1.getUrl)(url_utils_js_1.DEVELOPER_URL) ? `${(0, url_utils_js_1.getUrl)(url_utils_js_1.DEVELOPER_URL)}/consumer/cn/customerService` : undefined;
export const DEFAULT_MORE_INFO_URL_EN = (0, url_utils_js_1.getUrl)(url_utils_js_1.DEVELOPER_URL) ? `${(0, url_utils_js_1.getUrl)(url_utils_js_1.DEVELOPER_URL)}/consumer/en/customerService` : undefined;
export const ErrorCodeDescription = {
  [ErrorCode.ERROR_00]: "Unknown Error",
  [ErrorCode.ERROR_01]: "Dependency Error",
  [ErrorCode.ERROR_02]: "Script Error",
  [ErrorCode.ERROR_03]: "Configuration Error",
  [ErrorCode.ERROR_04]: "Not Found",
  [ErrorCode.ERROR_05]: "Syntax Error",
  [ErrorCode.ERROR_06]: "Specification Limit Violation",
  [ErrorCode.ERROR_07]: "Permissions Error",
  [ErrorCode.ERROR_08]: "Operation Error",
  [ErrorCode.ERROR_09]: "ArkTS Compiler Error"
};
(function (r) {
  r.CN = "cn", r.EN = "en";
})(CountryEnum = CountryEnum || (CountryEnum = {})), function (r) {
  r[r.COLLECT_LAST = 1] = "COLLECT_LAST", r[r.COLLECT_FIRST = 2] = "COLLECT_FIRST", r[r.COLLECT_ALL = 3] = "COLLECT_ALL";
}(MergeType = MergeType || (MergeType = {})), function (r) {
  r.CODE = "code", r.CAUSE = "cause", r.POSITION = "position", r.SOLUTIONS = "solutions", r.MORE_INFO = "moreInfo";
}(MergeKeyEnum = MergeKeyEnum || (MergeKeyEnum = {})), function (r) {
  r.UNKNOWN = "000", r.HVIGOR = "103", r.PACKAGE = "200";
}(SubSystemEnum = SubSystemEnum || (SubSystemEnum = {})), function (r) {
  r.DEBUG = "debug", r.INFO = "info", r.WARN = "warn", r.ERROR = "error";
}(LogLevel = LogLevel || (LogLevel = {})), function (r) {
  r.DEBUG = "debug", r.INFO = "info", r.WARN = "warn", r.ERROR = "error", r.DETAIL = "detail";
}(MetricLogType = MetricLogType || (MetricLogType = {})), function (r) {
  r.ERROR_00 = "00", r.ERROR_01 = "01", r.ERROR_02 = "02", r.ERROR_03 = "03", r.ERROR_04 = "04", r.ERROR_05 = "05", r.ERROR_06 = "06", r.ERROR_07 = "07", r.ERROR_08 = "08", r.ERROR_09 = "09";
}(ErrorCode = ErrorCode || (ErrorCode = {}));
