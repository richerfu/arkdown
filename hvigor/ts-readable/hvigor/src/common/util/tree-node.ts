/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/tree-node.js
 */
export class TreeNode {
  constructor(e, r) {
    this.key = e, this.value = r, this.children = [];
  }
}
export class Tree {
  constructor(e, r) {
    this.root = new TreeNode(e, r);
  }
  findNodeByKeyDFS(e, r) {
    if (r.key === e) return r;
    for (const o of r.children) {
      const r = this.findNodeByKeyDFS(e, o);
      if (r) return r;
    }
  }
  findNodeByKeyBFS(e) {
    const r = [this.root];
    for (; r.length;) {
      const o = r.shift();
      if (o.key === e) return o;
      r.push(...o.children);
    }
  }
  findParentNodeByKey(e) {
    const r = [[this.root, undefined]];
    for (; r.length;) {
      const [o, t] = r.shift();
      if (o.key === e) return t;
      for (const e of o.children) r.push([e, o]);
    }
  }
  addNode(e, r, o) {
    const t = this.findNodeByKeyBFS(o);
    if (!t) return false;
    const s = new TreeNode(e, r);
    return t.children.push(s), true;
  }
}
