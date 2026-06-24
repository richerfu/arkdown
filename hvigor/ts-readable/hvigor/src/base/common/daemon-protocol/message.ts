/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/daemon-protocol/message.js
 */
export function createOutput(t, e, u) {
  return {
    type: t,
    text: e,
    encoding: u
  };
}
export function createBuildStatus(t, e, u) {
  return {
    status: t,
    exitCode: e,
    reason: u
  };
}
