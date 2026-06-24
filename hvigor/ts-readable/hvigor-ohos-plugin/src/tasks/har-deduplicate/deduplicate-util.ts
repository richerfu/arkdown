/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har-deduplicate/deduplicate-util.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as ohmurl_utils_js_1 from "../../utils/ohmurl-utils";
const log = ohos_logger_js_1.OhosLogger.getLogger("DeduplicateUtil");
export function getFilteredDuplicateHarInfo(e) {
  const r = new Set(),
    t = new Set(),
    o = new Set();
  return Object.values(e).forEach(e => {
    e.isByteCodeHarDependency ? r.add(e.packageName) : (t.add(e.dependencyRootPath), e.isSODependency || o.add(e.dependencyRootPath));
  }), {
    byteCodeHarPkgNameSet: r,
    sourceHarPkgPathSet: t,
    sourceHarWithoutSoPkgPathSet: o
  };
}
export function mergeDeclarationEntry(e, r) {
  mergeArray(e, r, e => {
    log.debug(`Deduplicate declarationEntry add ${e}`);
  });
}
export function mergeCompileEntry(e, r) {
  mergeArray(e, r, e => {
    log.debug(`Deduplicate compileEntry add ${e}`);
  });
}
export function mergeDynamicImportLibInfo(e, r) {
  mergeObject(e, r, e => {
    log.debug(`Deduplicate dynamicImportLibInfo add ${e}`);
  });
}
export function mergeWorkers(e, r) {
  mergeArray(e, r, e => {
    log.debug(`Deduplicate workers add ${e}`);
  });
}
export function processByteCodeHarDeclarationEntry(e, r, t, o, a) {
  r.forEach(r => {
    if (!r.toLowerCase().startsWith(e.toLowerCase())) return;
    if (o.has(r.toLowerCase())) return;
    o.add(r.toLowerCase());
    const i = genByteCodeHarImportOhmUrl(r, t);
    i && a.add(i);
  });
}
function genByteCodeHarImportOhmUrl(e, r) {
  const t = e.toLowerCase(),
    o = r.packageName.toLowerCase();
  let a;
  if (t === o || new RegExp(`^${o}(/*|\\\\*)$`).test(t)) a = (0, ohmurl_utils_js_1.generateOhmUrl)(r);else if (t.startsWith(`${o}/`)) {
    const t = e.substring(`${o}/`.length).replace(/\\/g, "/");
    a = (0, ohmurl_utils_js_1.generateOhmUrlForSourceFile)(r, t);
  } else if (t.startsWith(`${o}\\`)) {
    const t = e.substring(`${o}\\`.length).replace(/\\/g, "/");
    a = (0, ohmurl_utils_js_1.generateOhmUrlForSourceFile)(r, t);
  }
  return a;
}
export function deduplicateWorkers(e, r) {
  return deduplicateArrayStartWith(e, r, e => {
    log.debug(`Deduplicate workers ${e}`);
  });
}
export function deduplicateCompileEntry(e, r) {
  return deduplicateArrayStartWith(e, r, e => {
    log.debug(`Deduplicate compileEntry ${e}`);
  });
}
export function deduplicateOtherCompileFiles(e, r) {
  return deduplicateArrayStartWith(e, r, e => {
    log.debug(`Deduplicate otherCompileFiles ${e}`);
  });
}
export function deduplicateDeclarationEntry(e, r) {
  const t = [];
  for (let o = r.length - 1; o >= 0; o--) {
    const a = r[o];
    (0, ohmurl_utils_js_1.transformOhmUrlToPkgName)(a) === e && (t.push(a), r.splice(o, 1), log.debug(`Deduplicate declarationEntry ${e} ${a}`));
  }
  return t;
}
export function deduplicateDynamicImportLibInfoObj(e, r) {
  const t = {};
  return Object.keys(e).forEach(o => {
    const a = e[o];
    r.has(a.pkgPath) && (t[o] = a, delete e[o], log.debug(`Deduplicate dynamicImportLibInfo ${o} : ${a}`));
  }), t;
}
export function deduplicateArray(e, r) {
  const t = [];
  for (let o = e.length - 1; o >= 0; o--) {
    const a = e[o];
    r(a) && (t.push(a), e.splice(o, 1));
  }
  return t;
}
export function mergeTwoArray(e, r, t, o = hvigor_1.noop) {
  e.forEach(e => {
    r.find(r => t(e, r)) || (r.push(e), o(e));
  });
}
function mergeArray(e, r, t) {
  e.forEach(e => {
    r.includes(e) || (r.push(e), t(e));
  });
}
function mergeObject(e, r, t) {
  Object.keys(e).forEach(o => {
    r[o] || (r[o] = e[o], t({
      [o]: e[o]
    }));
  });
}
function deduplicateArrayStartWith(e, r, t) {
  const o = [];
  for (let a = e.length - 1; a >= 0; a--) {
    const i = e[a];
    for (const n of r) if (i.startsWith(n)) {
      o.push(i), e.splice(a, 1), t(i);
      break;
    }
  }
  return o;
}
