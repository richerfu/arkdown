/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/log-collection/log.js
 */
import * as log_type_js_1 from "../enum/log-type";
export function generateScheduleLog(e, t, o) {
  return {
    type: log_type_js_1.LogType.SCHEDULE,
    time: e,
    workId: t,
    workerId: o
  };
}
