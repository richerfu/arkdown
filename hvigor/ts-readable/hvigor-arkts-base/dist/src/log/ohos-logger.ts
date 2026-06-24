/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/log/ohos-logger.js
 */
import os_1 from "os";
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
import * as arkts_base_log_js_1 from "./arkts-base-log";
export function addCompilerLogMsg(o) {
  let e = `${o}${os_1.EOL}`,
    r = 1,
    s = 0,
    t = 0,
    g = "",
    i = "",
    l = "";
  const a = "[31m",
    _ = "[39m";
  return arkts_base_log_js_1.ArktsBaseLogger.getLogEvents().forEach(o => {
    if ("error" === o.level) {
      const e = hvigor_logger_1.ErrorUtil.getFirstErrorAdaptorMessage(new hvigor_logger_1.ArkTsErrorAdaptor(String(o.msg)).getErrorMessage());
      e.solutions ? g += hvigor_logger_1.ErrorUtil.combinePhase({
        code: e.code,
        description: "",
        cause: `${a}${r} ERROR: ${e.message}`,
        position: "",
        solutions: e.solutions,
        moreInfo: e.moreInfo
      }) : g += `${a}${r} ERROR: ${e.message}`, g += `${os_1.EOL}${_}${os_1.EOL}`, r++;
    }
    "warn" === o.level && (s++, i += `${os_1.EOL}[33m${s} WARN: ${o.msg}${_}`), "info" === o.level && (t++, l += `${os_1.EOL}[34m${t} INFO: ${o.msg}${_}`);
  }), e = `${e}${g}${a}COMPILE RESULT:FAIL {ERROR:${r}` + (0 === s ? "" : ` WARN:${s}`) + `${0 === t ? "" : ` INFO:${t}`}}${_}`, {
    infoMsg: "" === l ? undefined : l,
    warnMsg: "" === i ? undefined : i,
    errorMsg: e
  };
}
