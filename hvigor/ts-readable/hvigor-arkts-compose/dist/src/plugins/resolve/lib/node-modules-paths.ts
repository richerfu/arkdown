/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/resolve/lib/node-modules-paths.js
 */
import path_1 from "path";
const parse = path_1.parse,
  getNodeModulesDirs = function (e, t) {
    let o = "/";
    /^([A-Za-z]:)/.test(e) ? o = "" : /^\\\\/.test(e) && (o = "\\\\");
    const s = [e];
    let r = parse(e);
    for (; r.dir !== s[s.length - 1];) s.push(r.dir), r = parse(r.dir);
    return s.reduce((e, s) => e.concat(t.map(e => path_1.resolve(o, s, e))), []);
  };
export function nodeModulesPaths(e, t, o) {
  const s = t && t.moduleDirectory ? [].concat(t.moduleDirectory) : ["oh_modules"];
  if (t && "function" == typeof t.paths) return t.paths(o, e, function () {
    return getNodeModulesDirs(e, s);
  }, t);
  const r = getNodeModulesDirs(e, s);
  return t && t.paths ? t.paths.concat(r) : r;
}
