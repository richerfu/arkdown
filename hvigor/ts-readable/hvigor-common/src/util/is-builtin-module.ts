/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/is-builtin-module.js
 */
import * as module_1 from "module";
function getCoreModule(e) {
  return module_1.builtinModules.filter(t => !/^_|^(internal|v8|node-inspect)\/|\//.test(t) && !["sys"].includes(t) && e === t).length > 0;
}
export function isBuiltinModule(e) {
  if (0 === e.length) return false;
  e.startsWith("node:") && (e = e.slice(5));
  const t = e.indexOf("/");
  return -1 !== t && t !== e.length - 1 && (e = e.slice(0, t)), getCoreModule(e);
}
