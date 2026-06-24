/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/enum/pool-state.js
 */
export var PoolState;
export const transitionMap = new Map([[PoolState.INIT, new Set([PoolState.RUNNING])], [PoolState.RUNNING, new Set([PoolState.CLOSE, PoolState.STOP])], [PoolState.CLOSE, new Set([PoolState.RUNNING, PoolState.TERMINATED])], [PoolState.STOP, new Set([PoolState.TERMINATED])], [PoolState.TERMINATED, new Set()]]);
(function (t) {
  t.INIT = "init", t.RUNNING = "running", t.STOP = "stop", t.CLOSE = "close", t.TERMINATED = "terminated";
})(PoolState = PoolState || (PoolState = {}));
