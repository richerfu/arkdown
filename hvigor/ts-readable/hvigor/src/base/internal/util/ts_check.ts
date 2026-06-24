/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/util/ts_check.js
 */
import fs_1 from "fs";
import process_1 from "process";
import typescript_1 from "typescript";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as compile_options_js_1 from "../../util/options/compile-options";
import * as time_util_js_1 from "../../util/time-util";
const log = hvigor_log_js_1.HvigorLogger.getLogger("ts-check");
export const hvigorfileTSRoots = new Set();
export const targetRoots = new Set();
export const targetModules = new Set();
export const targetModulesRoots = new Map();
const formatHost = {
  getCanonicalFileName: t => t,
  getCurrentDirectory: typescript_1.sys.getCurrentDirectory,
  getNewLine: () => typescript_1.sys.newLine
};
export function hvigorfileTypeCheck() {
  removeInvalidFilePaths();
  const t = process_1.hrtime(),
    e = typescript_1.createProgram([...targetRoots], compile_options_js_1.compilerOptions),
    o = typescript_1.getPreEmitDiagnostics(e),
    i = process_1.hrtime(t),
    r = (0, time_util_js_1.formatTime)(i);
  for (const t of o) log.error(formatDiagnostic(t, formatHost));
  o.length ? log.printErrorExit("TYPE_ERRORS", [[...targetRoots].join(", ")]) : log.info(`TYPE CHECK SUCCESSFUL in ${r}`);
}
function removeInvalidFilePaths() {
  const t = [];
  for (const e of targetRoots) fs_1.existsSync(e) || t.push(e);
  t.forEach(t => targetRoots.delete(t));
}
export function doCompilePluginTypeCheck(t) {
  const e = typescript_1.createProgram([t], {
      ...compile_options_js_1.compilerOptions,
      noImplicitAny: false,
      noImplicitReturns: false,
      alwaysStrict: false,
      declaration: false
    }),
    o = typescript_1.getPreEmitDiagnostics(e);
  if (o.length) {
    for (const t of o) log.error(formatDiagnostic(t, formatHost));
    log.printErrorExit("TYPE_ERRORS", [t]);
  }
}
export function formatDiagnostic(t, e, o) {
  const i = `${diagnosticCategoryName(t)} TS${t.code}: ${typescript_1.flattenDiagnosticMessageText(t.messageText, e.getNewLine())}${e.getNewLine()}`;
  if (t.file) {
    const {
      line: e,
      character: r
    } = typescript_1.getLineAndCharacterOfPosition(t.file, t.start);
    return `${i} at ${null != o ? o : t.file.fileName}:${e + 1}:${r + 1} `;
  }
  return i;
}
function diagnosticCategoryName(t, e = true) {
  const o = typescript_1.DiagnosticCategory[t.category];
  return e ? o.toLowerCase() : o;
}
export function setHvigorConfigFileTsRoot(t) {
  hvigorConfigFileTsRoot = t;
}
export { hvigorConfigFileTsRoot };
