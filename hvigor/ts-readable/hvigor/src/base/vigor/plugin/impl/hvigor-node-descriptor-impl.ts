/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/vigor/plugin/impl/hvigor-node-descriptor-impl.js
 */
import * as hvigor_trace_factory_js_1 from "../../../common/trace/hvigor-trace-factory";
var __decorate = this && this.__decorate || function (e, t, o, r) {
  var i,
    d = arguments.length,
    c = d < 3 ? t : null === r ? r = Object.getOwnPropertyDescriptor(t, o) : r;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) c = Reflect.decorate(e, t, o, r);else for (var s = e.length - 1; s >= 0; s--) (i = e[s]) && (c = (d < 3 ? i(c) : d > 3 ? i(t, o, c) : i(t, o)) || c);
  return d > 3 && c && Object.defineProperty(t, o, c), c;
};
export class HvigorNodeDescriptorImpl {
  constructor(e, t, o, r) {
    this.extraOptions = new Map(), this.name = e, this.srcPath = t, this.childrenNode = o, this.rootNode = r;
  }
  getChildNode() {
    return this._getChildNode();
  }
  getChildNodeInternal() {
    return this._getChildNode();
  }
  _getChildNode() {
    return this.childrenNode;
  }
  getRootNode() {
    return this._getRootNode();
  }
  getRootNodeInternal() {
    return this._getRootNode();
  }
  _getRootNode() {
    return undefined === this.rootNode ? this : this.rootNode;
  }
  setChildNode(e) {
    this.childrenNode = e;
  }
}
__decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeDescriptorImpl.prototype, "getChildNode", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeDescriptorImpl.prototype, "getRootNode", null);
