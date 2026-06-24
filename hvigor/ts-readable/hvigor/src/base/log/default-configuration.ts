/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/log/default-configuration.js
 */
import * as log4js_1 from "log4js";
import path_1 from "path";
import * as path_util_js_1 from "../../common/util/path-util";
import * as path_const_js_1 from "../../common/const/path-const";
export const logFilePath = () => {
  let e;
  try {
    e = path_util_js_1.PathUtil.getHvigorCacheDir();
  } catch {
    e = path_const_js_1.HVIGOR_PROJECT_CACHE_PATH;
  }
  return path_1.resolve(e, "./outputs/build-logs");
};
let configuration = {
  appenders: {
    debug: {
      type: "stdout",
      layout: {
        type: "pattern",
        pattern: "[%d] > hvigor %p %c %[%m%]"
      }
    },
    "debug-log-file": {
      type: "file",
      filename: path_1.resolve((0, logFilePath)(), "build.log"),
      maxLogSize: 2097152,
      backups: 9,
      encoding: "utf-8",
      level: "debug"
    },
    info: {
      type: "stdout",
      layout: {
        type: "pattern",
        pattern: "[%d] > hvigor %[%m%]"
      }
    },
    "no-pattern-info": {
      type: "stdout",
      layout: {
        type: "pattern",
        pattern: "%m"
      }
    },
    wrong: {
      type: "stderr",
      layout: {
        type: "pattern",
        pattern: "[%d] > hvigor %[%p: %m%]"
      }
    },
    "just-debug": {
      type: "logLevelFilter",
      appender: "debug",
      level: "debug",
      maxLevel: "debug"
    },
    "just-info": {
      type: "logLevelFilter",
      appender: "info",
      level: "info",
      maxLevel: "info"
    },
    "just-wrong": {
      type: "logLevelFilter",
      appender: "wrong",
      level: "warn",
      maxLevel: "error"
    }
  },
  categories: {
    default: {
      appenders: ["just-debug", "just-info", "just-wrong"],
      level: "debug"
    },
    "no-pattern-info": {
      appenders: ["no-pattern-info"],
      level: "info"
    },
    "debug-file": {
      appenders: ["debug-log-file"],
      level: "debug"
    }
  }
};
export const setConfiguration = e => {
  configuration = e;
};
export const getConfiguration = () => configuration;
export const updateConfiguration = () => {
  const e = configuration.appenders["debug-log-file"];
  return e && "filename" in e && (e.filename = path_1.resolve((0, logFilePath)(), "build.log")), configuration;
};
let contextLevel = log4js_1.levels.DEBUG;
export const setCategoriesLevel = (e, t) => {
  contextLevel = e;
  const l = configuration.categories;
  for (const o in l) (null == t ? undefined : t.includes(o)) || o.includes("file") || Object.prototype.hasOwnProperty.call(l, o) && (l[o].level = e.levelStr);
};
export const getLevel = () => contextLevel;
export const levelMap = new Map([["ALL", log4js_1.levels.ALL], ["MARK", log4js_1.levels.MARK], ["TRACE", log4js_1.levels.TRACE], ["DEBUG", log4js_1.levels.DEBUG], ["INFO", log4js_1.levels.INFO], ["WARN", log4js_1.levels.WARN], ["ERROR", log4js_1.levels.ERROR], ["FATAL", log4js_1.levels.FATAL], ["OFF", log4js_1.levels.OFF], ["all", log4js_1.levels.ALL], ["mark", log4js_1.levels.MARK], ["trace", log4js_1.levels.TRACE], ["debug", log4js_1.levels.DEBUG], ["info", log4js_1.levels.INFO], ["warn", log4js_1.levels.WARN], ["error", log4js_1.levels.ERROR], ["fatal", log4js_1.levels.FATAL], ["off", log4js_1.levels.OFF]]);
