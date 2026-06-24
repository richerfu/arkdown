/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/utils/hvigor-logger-config.js
 */
import * as log4js_1 from "log4js";
import path_1 from "path";
import * as log_path_util_js_1 from "./log-path-util";
export class HvigorLoggerConfig {
  static setExtraConfig(e) {
    HvigorLoggerConfig.extraConfig = e;
  }
  static getExtraConfig(e) {
    return HvigorLoggerConfig.extraConfig.get(e);
  }
  static setHvigorCacheDir(e) {
    HvigorLoggerConfig.hvigorCacheDir = e;
  }
  static getHvigorCacheDir() {
    return HvigorLoggerConfig.hvigorCacheDir;
  }
  static updateConfiguration() {
    const e = HvigorLoggerConfig.configuration.appenders["debug-log-file"];
    return e && "filename" in e && (e.filename = path_1.resolve(log_path_util_js_1.LogPathUtil.logFilePath(), "build.log")), HvigorLoggerConfig.configuration;
  }
  static setCategoriesLevel(e, o) {
    HvigorLoggerConfig.logLevel = e;
    const g = HvigorLoggerConfig.configuration.categories;
    for (const t in g) (null == o ? undefined : o.includes(t)) || t.includes("file") || Object.prototype.hasOwnProperty.call(g, t) && (g[t].level = e.levelStr);
  }
  static getLevel() {
    return HvigorLoggerConfig.logLevel;
  }
}
HvigorLoggerConfig.extraConfig = new Map(), HvigorLoggerConfig.configuration = {
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
      filename: path_1.resolve(log_path_util_js_1.LogPathUtil.logFilePath(), "build.log"),
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
}, HvigorLoggerConfig.logLevel = log4js_1.levels.DEBUG, HvigorLoggerConfig.setConfiguration = e => {
  HvigorLoggerConfig.configuration = e;
}, HvigorLoggerConfig.getConfiguration = () => HvigorLoggerConfig.configuration;
