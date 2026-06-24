/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/enum/task-state.js
 */
export var TaskState;
export const transitionMap = new Map([[TaskState.WAITING, new Set([TaskState.RUNNING, TaskState.REJECT])], [TaskState.RUNNING, new Set([TaskState.END, TaskState.ERROR])], [TaskState.END, new Set()], [TaskState.REJECT, new Set()], [TaskState.ERROR, new Set()]]);
(function (t) {
  t.WAITING = "waiting", t.RUNNING = "running", t.END = "end", t.REJECT = "reject", t.ERROR = "error";
})(TaskState = TaskState || (TaskState = {}));
