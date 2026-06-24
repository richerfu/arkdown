/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/incremental-decorator/util.js
 */
import * as hvigor_log_js_1 from "../log/hvigor-log";
const _toString = Object.prototype.toString,
  _hasOwn = Object.prototype.hasOwnProperty,
  DECLARATOR_CONTAINER = Symbol("DECLARATOR_CONTAINER");
export const incrementalDeclaratorLogger = hvigor_log_js_1.HvigorLogger.getLogger("incremental-declarator");
export const setSelfDeclaratorContainer = e => {
  _hasOwn.call(e, DECLARATOR_CONTAINER) || Object.defineProperty(e, DECLARATOR_CONTAINER, {
    value: [],
    configurable: false,
    enumerable: false
  });
};
export const getSelfDeclaratorContainer = e => {
  if (_hasOwn.call(e, DECLARATOR_CONTAINER)) return e[DECLARATOR_CONTAINER];
};
export const hasIncrementalDecorator = e => {
  var r;
  return !!(null === (r = e[DECLARATOR_CONTAINER]) || undefined === r ? undefined : r.length);
};
export const getType = e => _toString.call(e).slice(8, -1);
