/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/log-path-util.js
 */
import path_1 from "path";
import process_1 from "process";
import * as hvigor_logger_config_js_1 from "./hvigor-logger-config";
import * as hvigor_log_info_js_1 from "../logger/hvigor-log-info";
import * as errorlog_consts_js_1 from "../consts/errorlog-consts";
export class LogPathUtil {
  static logFilePath() {
    let o;
    try {
      o = LogPathUtil.getHvigorCacheDir();
    } catch {
      o = path_1.resolve(LogPathUtil.HVIGOR_PROJECT_ROOT_DIR, LogPathUtil.HVIGOR_USER_HOME_DIR_NAME);
    }
    return path_1.resolve(o, "./outputs/build-logs");
  }
  static getHvigorCacheDir(o) {
    var e;
    let r = undefined !== process_1.env.config ? JSON.parse(process_1.env.config)[errorlog_consts_js_1.BUILD_CACHE_DIR] : null !== (e = hvigor_logger_config_js_1.HvigorLoggerConfig.getExtraConfig(errorlog_consts_js_1.BUILD_CACHE_DIR)) && undefined !== e ? e : LogPathUtil.getCommandHvigorCacheDir();
    const t = path_1.resolve(LogPathUtil.HVIGOR_PROJECT_ROOT_DIR, LogPathUtil.HVIGOR_USER_HOME_DIR_NAME);
    return r || (r = hvigor_logger_config_js_1.HvigorLoggerConfig.getHvigorCacheDir(), r) ? path_1.isAbsolute(r) ? (o && !this.hvigorCacheDirHasLogged && (o.warn(new hvigor_log_info_js_1.HvigorLogInfo().setLogLevel(errorlog_consts_js_1.LogLevel.WARN).setMessage("Please ensure no projects of the same name have the same custom hvigor data dir.")), this.hvigorCacheDirHasLogged = true), path_1.resolve(r, path_1.basename(process_1.cwd()), errorlog_consts_js_1.HVIGOR_USER_HOME_DIR_NAME)) : (o && !this.hvigorCacheDirHasLogged && (o.warn(new hvigor_log_info_js_1.HvigorLogInfo().setLogLevel(errorlog_consts_js_1.LogLevel.WARN).setMessage(`Invalid custom hvigor data dir:${r}`)), this.hvigorCacheDirHasLogged = true), t) : t;
  }
  static getCommandHvigorCacheDir() {
    return process_1.argv.forEach(o => {
      o.startsWith(errorlog_consts_js_1.BUILD_CACHE_DIR) && (process_1.env.BUILD_CACHE_DIR = o.substring(o.indexOf("=") + 1));
    }), process_1.env.BUILD_CACHE_DIR;
  }
}
LogPathUtil.HVIGOR_PROJECT_ROOT_DIR = process_1.cwd(), LogPathUtil.HVIGOR_USER_HOME_DIR_NAME = ".hvigor", LogPathUtil.hvigorCacheDirHasLogged = false;
