/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/match-task-with-node.js
 */
const taskWithNodeRex = /^(?<node>.+?):(?<taskName>.+)$/;
export function matchTaskWithNode(e) {
  const t = taskWithNodeRex.exec(e);
  if (t && t.groups) {
    const {
      node: e,
      taskName: o
    } = t.groups;
    return {
      node: e,
      taskName: o
    };
  }
}
