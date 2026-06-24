/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/build-steps/process-importee.js
 */
import path_1 from "path";
import * as fse from "fs-extra";
import * as constants_js_1 from "../util/constants";
export async function processImportees(e, t, r, s, o) {
  const {
    isByteCodeHar: n,
    bundledDependencies: i,
    shouldCollectDirectImportees: a
  } = t;
  if (!n && !a || !r || !s || !o) return;
  const _ = e.startEvent("process importees"),
    [u, c, l] = collectImportees(r, o);
  n && (await fse.writeJson(path_1.join(s, constants_js_1.NON_ENTRY_IMPORTEE_CACHE_FILE_NAME), u)), i && (await fse.writeJson(path_1.join(s, constants_js_1.COMPILED_IMPORTEE_CACHE_FILE_NAME), c)), a && (await fse.writeJson(path_1.join(s, constants_js_1.DIRECT_IMPORTEE_CACHE_FILE_NAME), l)), e.endEvent(_.getId());
}
const isValidImportee = (e, t = true) => {
  if (!e || /\0/.test(e) || path_1.isAbsolute(e) || e.startsWith(".")) return false;
  if (!t) return true;
  const r = e.startsWith("@");
  return e.split(/[/\\]/).length > (r ? 2 : 1);
};
function collectImportees(e, t) {
  const r = new Set(),
    s = new Set(),
    o = new Set();
  return t.modules.forEach(t => {
    Object.keys(t.resolvedIds).forEach(n => {
      isValidImportee(n, false) && s.add(n), t.meta?.belongModulePath === e && (isValidImportee(n) && r.add(n), isValidImportee(n, false) && o.add(n));
    });
  }), [Array.from(r), Array.from(s), Array.from(o)];
}
