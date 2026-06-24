/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/enum/worker-state.js
 */
export var WorkerState;
(function (e) {
  e[e.IDLE = 0] = "IDLE", e[e.BUSY = 1] = "BUSY", e[e.DEAD = 2] = "DEAD";
})(WorkerState = WorkerState || (WorkerState = {}));
