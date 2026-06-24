/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract-build-native.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
import * as child_process_1 from "child_process";
import * as fs_1 from "fs";
import * as promises_1 from "node:fs/promises";
import * as node_readline_1 from "node:readline";
import os_1 from "os";
import path_1 from "path";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as clang_log_diagnostic_js_1 from "../utils/log/clang-log-diagnostic";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
const logger = ohos_logger_js_1.OhosLogger.getLogger("Native"),
  ErrorFlags = new Map([["error: undefined symbol: ", "A_UNDEFINED_SYMBOL_ERROR_HAS_OCCURRED"], ["error: unknown type name ", "A_UNKNOWN_TYPE_NAME_ERROR_HAS_OCCURRED"]]);
export class AbstractBuildNative extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e, o) {
    super(e, o), this._nativeOption = e.getBuildOption().externalNativeOptions;
  }
  async executeCommand(e, o, t, r, i) {
    const n = {
        commandLine: e,
        logFolder: o
      },
      s = {
        workInput: n,
        callback: t,
        callbackInput: r
      },
      a = path_1.resolve(__filename, "nativeExecution");
    if (this.getWorkerPool().submit(this, a, s).getState() === hvigor_1.TaskState.REJECT) {
      const e = "execute the command",
        o = i.createSubEvent(e, "");
      o.start(), await (0, nativeExecution)(n), await t(...r), o.stop(), o.setLog(e, hvigor_1.MetricLogType.INFO);
    }
  }
}
export const nativeExecution = async (e, o) => {
  var t, r, i;
  null == o || o.mount("nativeExecution").setCache("args", e);
  const n = (0, hvigor_1.isWindows)() ? "GBK" : "UTF-8",
    s = path_1.resolve(e.logFolder, "output.log"),
    a = (0, child_process_1.spawn)(e.commandLine[0], e.commandLine.slice(1), {
      windowsHide: true
    }),
    l = (0, fs_1.createWriteStream)(s, {});
  let c = [],
    _ = [];
  const g = null !== (t = e.commandLine[0]) && undefined !== t ? t : "",
    u = g.endsWith("ninja") || g.endsWith("ninja.exe");
  return null === (r = a.stdout) || undefined === r || r.on("data", e => {
    const o = hvigor_1.iconv.decode(e, n);
    if (l.write(o), c.push(o), o.endsWith("\r") || o.endsWith("\n")) {
      const e = c.join("");
      logger.debug(e), u && -1 !== e.indexOf("-Wunguarded-availability") && logger.warn(e), c = [];
    }
  }), null === (i = a.stderr) || undefined === i || i.on("data", e => {
    const o = hvigor_1.iconv.decode(e, n);
    l.write(o), _.push(o), (o.endsWith("\r") || o.endsWith("\n")) && (logger.warn(_.join("")), _ = []);
  }), new Promise((o, t) => {
    a.once("close", async r => {
      var i;
      if (l.end(), 0 === r) o();else {
        const o = null !== (i = e.commandLine[0]) && undefined !== i ? i : "",
          r = o.endsWith("cmake") || o.endsWith("cmake.exe");
        let n = `Exceptions happened while executing ${e.commandLine.join(" ")}.`;
        const a = await readException(s, r);
        n += a ? os_1.EOL + a : "", t(new Error(n));
      }
    });
  });
};
const readException = async (e, o) => {
    var t;
    try {
      if (o) {
        let o = await (0, promises_1.readFile)(e, {
          encoding: "utf8"
        });
        return o += checkSolution(o), o;
      }
      const r = new clang_log_diagnostic_js_1.ClangLogHandler(),
        i = (0, node_readline_1.createInterface)({
          input: (0, fs_1.createReadStream)(e),
          crlfDelay: 1 / 0
        });
      for await (const e of i) r.log(e);
      return null !== (t = r.text + checkSolution(r.text)) && undefined !== t ? t : "";
    } catch (o) {
      return `Log parsing failed. Please check the log file at ${e}.`;
    }
  },
  checkSolution = e => {
    var o;
    let t;
    for (const [r, i] of ErrorFlags.entries()) if (e.includes(r)) {
      const e = hvigor_logger_1.ErrorUtil.getFirstErrorAdaptorMessage(new hvigor_logger_1.HvigorOhosPluginAdaptor(i).getErrorMessage());
      return t = `${e.message}${os_1.EOL}* Try the following:${os_1.EOL}`, t += `  > ${null === (o = e.solutions) || undefined === o ? undefined : o.join(" ")}\n`, e.moreInfo && (t += `  > More info: ${e.moreInfo[(0, hvigor_common_1.getOsLanguage)()]}`), `[31m${t}[31m[39m`;
    }
    return t;
  };
