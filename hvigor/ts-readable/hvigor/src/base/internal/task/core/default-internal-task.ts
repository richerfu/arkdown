/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/default-internal-task.js
 */
import * as core_task_impl_js_1 from "../../../external/task/core-task-impl";
export class DefaultInternalTask extends core_task_impl_js_1.CoreTaskImpl {
  constructor(e, s, t) {
    super(e, s), this.fn = t;
  }
}
