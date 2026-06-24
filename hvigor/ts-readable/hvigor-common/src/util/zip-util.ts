/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/zip-util.js
 */
import fs_1 from "fs";
import zip_js_1 from "@zip.js/zip";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
export function getZipReader(e) {
  return new zip_js_1.ZipReader(new zip_js_1.BlobReader(e));
}
export function getZipWriter(e) {
  return new zip_js_1.ZipWriter(new zip_js_1.BlobWriter(e));
}
export function getUint8ArrayReader(e) {
  return new zip_js_1.Uint8ArrayReader(new Uint8Array(fs_extra_1.readFileSync(e)));
}
export async function addFilesToZip(e, t, i, r = "") {
  const a = fs_extra_1.readdirSync(t, {
    withFileTypes: true
  });
  for (const n of a) {
    const a = path_1.join(t, n.name),
      s = path_1.posix.join(r, n.name);
    if (n.isSymbolicLink()) {
      const t = fs_extra_1.realpathSync(a);
      if (i.has(t)) continue;
      i.add(t), await addFilesToZip(e, t, i, s);
    } else n.isDirectory() ? await addFilesToZip(e, a, i, s) : n.isFile() && (await e.add(s, getUint8ArrayReader(a)));
  }
}
export async function readFileInZIP(e, t) {
  const i = new Map(),
    r = fs_1.readFileSync(e),
    a = getZipReader(new Blob([r]));
  try {
    const e = await a.getEntries();
    for (const r of e) if (!r.directory && t.includes(r.filename)) {
      const e = r.getData && (await r.getData(new zip_js_1.TextWriter()));
      i.set(r.filename, e || "");
    }
  } finally {
    a && (await a.close());
  }
  return i;
}
