/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/task/core/task-directed-acyclic-graph.js
 */
import * as multi_map_js_1 from "../../../../common/util/multi-map";
export class TaskDirectedAcyclicGraph {
  constructor() {
    this._out = new multi_map_js_1.MultiMap(), this._in = new multi_map_js_1.MultiMap();
  }
  addNode(e) {
    this._out.put(e), this._in.put(e);
  }
  addEdge(e, t) {
    return this._out.put(e, t), this._out.put(t), this._in.put(t, e), this._in.put(e), true;
  }
  _hasEdge(e, t) {
    if (e === t) return true;
    const r = this._out.get(e);
    for (const e of r) if (this._hasEdge(e, t)) return true;
    return false;
  }
  removeNode(e) {
    const t = this._out.removeAll(e);
    for (const r of t) this._in.remove(r, e);
    const r = this._in.removeAll(e);
    for (const t of r) this._out.remove(t, e);
  }
  clear() {
    this._out.clear(), this._in.clear();
  }
  getChildren(e) {
    return this._out.get(e);
  }
  getParent(e) {
    return this._in.get(e);
  }
  getAllStartNodes() {
    return this.findAllZeroEdgeNodes(this._in);
  }
  getAllEndNodes() {
    return this.findAllZeroEdgeNodes(this._out);
  }
  checkCircle() {
    const e = new Set(),
      t = new Set(),
      r = this._out.keys(),
      i = {};
    for (const s of r) if (this.dfsCircleCheck(s, e, t, i)) return {
      hasCircle: true,
      circlePath: [...t, i.node]
    };
    return {
      hasCircle: false,
      circlePath: []
    };
  }
  dfsCircleCheck(e, t, r, i) {
    if (r.has(e)) return i.node = e, true;
    if (t.has(e)) return false;
    t.add(e), r.add(e);
    for (const s of this.getChildren(e)) if (this.dfsCircleCheck(s, t, r, i)) return true;
    return r.delete(e), false;
  }
  findAllZeroEdgeNodes(e) {
    const t = new Set();
    return e.keys().forEach(r => {
      0 === e.get(r).size && t.add(r);
    }), t;
  }
}
export const projectTaskDag = new TaskDirectedAcyclicGraph();
