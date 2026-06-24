/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/incremental-decorator/factory.js
 */
import * as util_js_1 from "./util";
export const createBaseDecorator = e => () => (r, t) => {
  (0, util_js_1.setSelfDeclaratorContainer)(r), (0, util_js_1.getSelfDeclaratorContainer)(r).push({
    propertyKey: t,
    declaratorType: e
  });
};
export const createFileDecorator = e => r => (t, o) => {
  (0, util_js_1.setSelfDeclaratorContainer)(t), (0, util_js_1.getSelfDeclaratorContainer)(t).push({
    propertyKey: o,
    declaratorType: e,
    options: r
  });
};
