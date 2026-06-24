/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/job.js
 */
import crypto_1 from "crypto";
import fs_1 from "fs";
import os_1 from "os";
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import * as log4js_1 from "log4js";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as generate_pkg_context_info_js_1 from "../generate-pkg-context-info";
import * as util_js_1 from "./util";
const startTime = Number(process.hrtime.bigint()),
  endTime = Number(process.hrtime.bigint()),
  log = ohos_logger_js_1.OhosLogger.getLogger("Ark");
let level = log4js_1.levels.DEBUG,
  hasLoadDependencies = false;
export const run = async (e, o) => ((0, util_js_1.generateLoaderEnv)(e), level = log4js_1.levels.getLevel(e.level, log4js_1.levels.INFO), log.setLevel(level), e.pkgContextInfo && (e.pkgContextInfo = new Proxy(e.pkgContextInfo, {
  get(e, o) {
    if ("string" == typeof o) return generate_pkg_context_info_js_1.soLibsPattern.test(o) && !e[o] ? (0, generate_pkg_context_info_js_1.getSoDefaultContextInfo)(o) : e[o];
  }
})), new Promise(t => {
  (0, hvigor_arkts_compose_1.runArkPack)(e, log, undefined, undefined, o).then(o => {
    o.isSuccess ? ((0, ohos_logger_js_1.handleLogs)(o.compileLogEvents, log, level), !e.isMainThread && !hasLoadDependencies && o.compileEvents.push(getLoadDependencyEvent()), t(o.compileEvents)) : t(o.error);
  }).catch(o => {
    var t;
    const r = path_1.resolve(null !== (t = e.cachePath) && undefined !== t ? t : "", hvigor_arkts_compose_1.MSGPACK_COMPILE_CACHE_DIR_NAME);
    throw fs_1.rmSync(r, {
      recursive: true,
      force: true
    }), o;
  }).finally(() => {
    log.debug(`Ark compile task finished.finished time is ${process.hrtime.bigint()}`);
  });
}));
export const runArkLint = async e => {
  (0, util_js_1.generateLoaderEnv)(e.config);
  const o = await (0, hvigor_arkts_compose_1.runEtsStandaloneChecker)(e);
  level = log4js_1.levels.getLevel(e.config.level, log4js_1.levels.INFO), (0, ohos_logger_js_1.handleLogs)(o, log, level);
  const t = o.filter(e => "error" === e.level);
  if (t.length > 0) {
    const e = t.map(e => e.msg).join(os_1.EOL);
    log._buildError(`ArkTS lint errors.${os_1.EOL}${e}`)._printErrorAndExit(), log.printErrorExit("ARKTS_LINT_ERRORS", [e]);
  }
};
function getLoadDependencyEvent() {
  return hasLoadDependencies = true, new hvigor_arkts_compose_1.CompileEvent(crypto_1.randomUUID(), "load compilation dependencies", process.pid, hvigor_1.MAIN_THREAD, 0, 0).setStartTime(startTime).setEndTime(endTime).setState("success");
}
