/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/har-deduplicate/hsp-collect-har-files.js
 */
import fs_1 from "fs";
import os_1 from "os";
import path_1 from "path";
export function hspCollectHarFilesPlugin() {
  return {
    name: "hspCollectHarFiles",
    buildEnd: buildEnd
  };
}
function buildEnd() {
  const e = this.share.projectConfig.hspDeduplicateHarInfo;
  if (!e || 0 === Object.keys(e).length) return;
  const t = new Set(),
    o = new Set();
  Object.values(e).forEach(e => {
    e.isByteCodeHarDependency ? o.add(e.packageName) : t.add(e.dependencyRootPath);
  });
  const s = [],
    r = [];
  collectSourceFiles(this, t, o, s, r);
  const i = this.share.projectConfig.intermediateHarDuplicateDirPath;
  if (fs_1.existsSync(i) || fs_1.mkdirSync(i, {
    recursive: true
  }), s.length > 0) {
    const e = path_1.join(i, SOURCE_HAR_DUPLICATE_FILES_INFO);
    fs_1.writeFileSync(e, s.join(os_1.EOL));
  }
  if (r.length > 0) {
    const e = path_1.join(i, BYTE_CODE_HAR_DUPLICATE_FILES_INFO);
    fs_1.writeFileSync(e, r.join(os_1.EOL));
  }
}
function collectSourceFiles(e, t, o, s, r) {
  for (const i of e.getModuleIds()) {
    const n = e.getModuleInfo(i);
    if (!n) continue;
    if (i.startsWith("\0")) continue;
    const l = collectByteCodeHar(n, o);
    if (l) r.push(l);else {
      const e = collectSourceHar(i, t);
      e && s.push(e);
    }
  }
}
function collectByteCodeHar(e, t) {
  if (!e.isExternal) return;
  const o = e.id,
    s = o.toLowerCase();
  for (const e of t) {
    const t = e.toLowerCase();
    if (s === t || s.startsWith(`${t}/`) || s.startsWith(`${t}\\`)) return o;
  }
}
function collectSourceHar(e, t) {
  for (const o of t) if (e.startsWith(o + path_1.sep)) return e;
}
export const SOURCE_HAR_DUPLICATE_FILES_INFO = "sourceHarDuplicateFilesInfo.txt";
export const BYTE_CODE_HAR_DUPLICATE_FILES_INFO = "byteCodeHarDuplicateFilesInfo.txt";
