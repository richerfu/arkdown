/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/cache.js
 */
import fs_1 from "fs";
import * as fs_js_1 from "../../util/fs";
import path_1 from "path";
export const caseErrorSet = new Set();
const onError = e => {
    if ("ENOENT" === e.code) return false;
    throw e;
  },
  makeCache = e => {
    const t = new Map(),
      r = async (r, a, i) => {
        t.has(r) || t.set(r, e(r).catch(e => {
          throw t.delete(r), e;
        }));
        try {
          const e = await t.get(r);
          return addCaseSensitiveErrorLog(r, e, i), await a(null, e);
        } catch (e) {
          return a(e);
        }
      };
    return r.clear = () => t.clear(), r;
  };
export const isDirCached = makeCache(async e => {
  try {
    const t = await (0, fs_js_1.stat)(e);
    return t.isDirectory();
  } catch (e) {
    return onError(e);
  }
});
export const isFileCached = makeCache(async e => {
  try {
    const t = await (0, fs_js_1.stat)(e);
    return t.isFile();
  } catch (e) {
    return onError(e);
  }
});
export const readCachedFile = makeCache(fs_js_1.readFile);
const legalDirectorySet = new Set(),
  notLegalDirectorySet = new Set();
let dirFileMap = {};
export function clearStepAddressingCache() {
  legalDirectorySet.clear(), notLegalDirectorySet.clear(), dirFileMap = {}, caseErrorSet.clear();
}
function getRootDir(e) {
  return e.startsWith("/") ? "/" : e.split(":\\")[0] + ":\\";
}
function checkFileWithCase(e, t, r) {
  try {
    t = getRootDir(t);
  } catch (e) {}
  if (legalDirectorySet.has(e) || [...r, t].includes(e)) return true;
  if (notLegalDirectorySet.has(e)) return false;
  const a = path_1.dirname(e),
    i = path_1.basename(e);
  let s;
  if (dirFileMap[a] ? s = dirFileMap[a] : (s = fs_1.readdirSync(a), dirFileMap[a] = s), !s.includes(i)) return notLegalDirectorySet.add(e), false;
  const o = checkFileWithCase(a, t, r);
  return o && legalDirectorySet.add(e), o;
}
function addCaseSensitiveErrorLog(e, t, r) {
  if (r?.caseSensitiveCheck && t) {
    const t = checkFileWithCase(e.toString(), r.rootDir, r.sdkModulesPath),
      a = r.originImportPath ?? e;
    if (a.endsWith("oh-package.json5") && r.joinPackageJson) return;
    if (!t) {
      const e = `ERROR File: ${r.importer} \n        Cannot resolve import statement ${r?.joinIndex ? `${a}/index` : a}. Please check the case.`;
      caseErrorSet.add(e);
    }
  }
}
