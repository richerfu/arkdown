/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/build-steps/rm-declared-files.js
 */
import path_1 from "path";
import * as fse from "fs-extra";
export function getModuleLastCachedSourceFiles(e) {
  if (!e.declaredFilesPath || !e.cache || !e.modulePath) return [];
  const t = e.startEvent("get module last cached source files"),
    r = collectFilesByModulePath(e.cache, e.modulePath);
  return e.endEvent(t.getId()), r;
}
export async function removeRedundantDeclaredFiles(e, t, r, n, a) {
  if (!(r && n && a && t.length)) return;
  const o = e.startEvent("remove redundant declared files"),
    i = difference(t, collectFilesByModulePath(a, n));
  if (!i.length) return void e.endEvent(o.getId());
  const l = new Set();
  for (const e of i) {
    const t = getDeclaredFilePath(r, e, n);
    fse.existsSync(t) && fse.realpathSync.native(t) === t && (await fse.remove(t), l.add(path_1.dirname(t)));
  }
  for (const e of l) fse.existsSync(e) && (await removeEmptyDir(e, r));
  e.endEvent(o.getId());
}
function collectFilesByModulePath(e, t) {
  if (!e.modules?.length) return [];
  const r = /(\.e?ts)$/;
  return e.modules.filter(e => e.meta?.belongModulePath === t && r.test(e.id)).map(e => e.id);
}
function difference(e, t) {
  const r = new Set(t);
  return e.filter(e => !r.has(e));
}
function getDeclaredFilePath(e, t, r) {
  const n = path_1.resolve(e, path_1.relative(r, t));
  return /(\.d\.e?ts)$/.test(n) ? n : n.replace(/(\.e?ts)$/, ".d$1");
}
async function removeEmptyDir(e, t) {
  const r = path_1.relative(t, e);
  if (!r) return;
  const n = r.split(path_1.sep);
  for (; n.length;) {
    const e = path_1.resolve(t, ...n);
    if (!fse.existsSync(e)) return;
    if ((await fse.readdir(e)).length) return;
    await fse.remove(e), n.pop();
  }
}
