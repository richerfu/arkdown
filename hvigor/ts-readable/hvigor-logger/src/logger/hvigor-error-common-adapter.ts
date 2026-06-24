/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/logger/hvigor-error-common-adapter.js
 */
import os_1 from "os";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as hvigor_log_info_js_1 from "./hvigor-log-info";
import * as errorlog_consts_js_1 from "../consts/errorlog-consts";
export class HvigorErrorCommonAdapter {
  convertErrorToLogInfo(o) {
    return new hvigor_log_info_js_1.HvigorLogInfo({
      message: HvigorErrorCommonAdapter.red + this.combinePhase(o) + HvigorErrorCommonAdapter.reset,
      logLevel: errorlog_consts_js_1.LogLevel.ERROR
    });
  }
  convertWarnToLogInfo(o) {
    return new hvigor_log_info_js_1.HvigorLogInfo({
      message: HvigorErrorCommonAdapter.yellow + o + HvigorErrorCommonAdapter.reset,
      logLevel: errorlog_consts_js_1.LogLevel.WARN
    });
  }
  convertInfoToLogInfo(o) {
    return new hvigor_log_info_js_1.HvigorLogInfo({
      message: HvigorErrorCommonAdapter.green + o + HvigorErrorCommonAdapter.reset,
      logLevel: errorlog_consts_js_1.LogLevel.INFO
    });
  }
  convertDebugToLogInfo(o) {
    return new hvigor_log_info_js_1.HvigorLogInfo({
      message: o,
      logLevel: errorlog_consts_js_1.LogLevel.DEBUG
    });
  }
  combinePhase(o) {
    return this.combinePhase1(o) + `${os_1.EOL}` + this.combinePhase2(o) + `${os_1.EOL}` + this.combinePhase3(o);
  }
  combinePhase1(o) {
    return `${o.getCode()} ${o.getDescription()}`;
  }
  combinePhase2(o) {
    const r = " At ",
      e = r + o.getPosition();
    return `Error Message: ${o.getCause()}${e === r ? "" : e}`.includes(errorlog_consts_js_1.SPLIT_TAG) ? this.composeCauseAndPosition(o.getCause(), o.getPosition()) : `Error Message: ${o.getCause()}${e === r ? "" : e}`;
  }
  composeCauseAndPosition(o, r) {
    let e = "Error Message: ";
    const t = " At ",
      s = o.split(errorlog_consts_js_1.SPLIT_TAG),
      n = r.split(errorlog_consts_js_1.SPLIT_TAG);
    return e = s.length === n.length ? this.composeCauseAndPositionWithSameLength(s, n, t, e) : this.composeCauseAndPositionWithUnSameLength(s, n, ` ${t}`, e), e;
  }
  composeCauseAndPositionWithSameLength(o, r, e, t) {
    for (let s = 0; s < o.length; s++) {
      t += o[s] + (e + r[s]) + `${os_1.EOL}`;
    }
    return t.slice(0, -`${os_1.EOL}`.length);
  }
  composeCauseAndPositionWithUnSameLength(o, r, e, t) {
    for (let r = 0; r < o.length; r++) {
      t += o[r] + `${os_1.EOL}`;
    }
    for (let o = 0; o < r.length; o++) {
      t += e + r[o] + `${os_1.EOL}`;
    }
    return t.slice(0, -`${os_1.EOL}`.length);
  }
  combinePhase3(o) {
    let r = `${os_1.EOL}* Try the following:${os_1.EOL}`;
    const e = o.getSolutions();
    if (!(e instanceof Array && e.length > 0)) return "";
    e.forEach(o => {
      r += `  > ${o}${os_1.EOL}`;
    });
    const t = o.getMoreInfo();
    if (t) {
      const o = t[(0, hvigor_common_1.getOsLanguage)()];
      r += `  > More info: ${o}${os_1.EOL}`;
    }
    return r;
  }
  static combineTrySuggestion(o, r) {
    const e = [`${os_1.EOL}${os_1.EOL}${HvigorErrorCommonAdapter.red}${HvigorErrorCommonAdapter.TRY_SUGGESTION_FIRST_LINE}${os_1.EOL}`];
    return o || e.push(`${HvigorErrorCommonAdapter.TRY_SUGGESTION_STACKTRACE_LINE}${os_1.EOL}`), r || e.push(`${HvigorErrorCommonAdapter.TRY_SUGGESTION_DEBUG_LINE}${os_1.EOL}`), e.length > 1 ? e.join("") : "";
  }
}
HvigorErrorCommonAdapter.red = "[31m", HvigorErrorCommonAdapter.green = "[32m", HvigorErrorCommonAdapter.yellow = "[33m", HvigorErrorCommonAdapter.blue = "[34m", HvigorErrorCommonAdapter.reset = "[39m", HvigorErrorCommonAdapter.TRY_SUGGESTION_FIRST_LINE = "* Try:", HvigorErrorCommonAdapter.TRY_SUGGESTION_STACKTRACE_LINE = "> Run with --stacktrace option to get the stack trace.", HvigorErrorCommonAdapter.TRY_SUGGESTION_DEBUG_LINE = "> Run with --debug option to get more log output.";
