/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/default-sync-task.js
 */
import * as core_task_js_1 from "../../../external/task/core-task";
import * as default_task_js_1 from "../../../external/task/default-task";
export class DefaultSyncTask extends default_task_js_1.DefaultTask {
  constructor(e, s) {
    super(e, s), this.group = core_task_js_1.HvigorTaskGroupType.SYNC_TASK_GROUP;
  }
}
