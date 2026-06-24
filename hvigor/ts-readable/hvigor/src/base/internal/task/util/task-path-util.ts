/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/util/task-path-util.js
 */
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
const log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-task-path"),
  _keySeparator = ":";
export function validateTaskPath(a) {
  const t = a.split(":");
  return t.length < 2 && log.printErrorExit("INVALID_TASK_NODE_KEY", [a]), t;
}
export function getModuleNameFromTaskPath(a) {
  return validateTaskPath(a)[0];
}
export function getTaskNameFromTaskPath(a) {
  return validateTaskPath(a)[1];
}
export function union(a, t) {
  return a.concat(":").concat(t);
}
export const parseTaskPath = a => {
  const t = validateTaskPath(a);
  return {
    moduleName: t[0],
    taskName: t[1]
  };
};
