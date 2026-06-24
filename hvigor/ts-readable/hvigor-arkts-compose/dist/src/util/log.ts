/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/log.js
 */
import * as hvigor_logger_1 from "@ohos/hvigor-logger";
export function hvigorConsoleLoggerPrintError(r, o, e, g) {
  r.printError(formErrorMsg(o, e, g));
}
export function hvigorConsoleLoggerPrintErrorAndExit(r, o, e, g) {
  r.printErrorAndExit(formErrorMsg(o, e, g));
}
function formErrorMsg(r, o, e) {
  const g = (0, hvigor_logger_1.formatErrorAdaptorExport)(r, o, e).getErrorMessage()[0] ?? {
    message: ""
  };
  return {
    code: g.code ?? "00000000",
    cause: g.message,
    description: (0, hvigor_logger_1.errorCode2Description)(g.code ?? "00000000"),
    solutions: g.solutions
  };
}
