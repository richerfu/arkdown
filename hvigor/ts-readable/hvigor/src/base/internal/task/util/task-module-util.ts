/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/util/task-module-util.js
 */
import * as hvigor_core_js_1 from "../../../external/core/hvigor-core";
import * as task_path_util_js_1 from "./task-path-util";
export function getModuleFromTaskPath(e) {
  const t = (0, task_path_util_js_1.getModuleNameFromTaskPath)(e),
    o = hvigor_core_js_1.hvigorCore.getProject();
  let r;
  return r = "" === t ? o : o.findModuleByName(t), r;
}
