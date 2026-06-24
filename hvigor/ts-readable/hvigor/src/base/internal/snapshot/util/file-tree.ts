/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/util/file-tree.js
 */
import * as FS from "fs";
import * as PATH from "path";
import * as file_type_js_1 from "./file-type";
function safeReadDirSync(e) {
  let i = [];
  try {
    i = FS.readdirSync(e);
  } catch (e) {
    if ("EACCES" !== e.code && "EPERM" !== e.code) throw e;
  }
  return i;
}
function normalizePath(e) {
  return e.replace(/\\/g, "/");
}
export class FileInfo {
  constructor(e, i, t = file_type_js_1.FileType.UNKNOWN, r = 0, n = 0, o = false) {
    this.isSymbolicLink = false, this.name = e, this.path = i, this.type = t, this.size = r, this.lastModifiedTime = n, this.isSymbolicLink = o;
  }
  toString() {
    return `{name:${this.name},path:${this.path},type:${this.type},\n        size:${this.size},lastModifiedTime:${this.lastModifiedTime},isSymbolicLink:${this.isSymbolicLink}`;
  }
}
function walkFile(e, i, t, r = {}) {
  const n = PATH.extname(e).toLowerCase();
  r.extensions && !r.extensions.test(n) || t.visitFile(i);
}
function walkDirectory(e, i, t, r, n = {}, o) {
  const s = safeReadDirSync(e);
  if (undefined === n.maxDepth || n.maxDepth > i) {
    r.preVisitDirectory(t);
    for (const t of s) walkFileRecursive(PATH.join(e, t), i + 1, r, n, o);
    r.postVisitDirectory(t);
  }
}
function walkFileRecursive(e, i = 0, t, r = {}, n) {
  let o, s;
  e = r.normalizePath ? normalizePath(e) : e;
  try {
    o = FS.statSync(e), s = FS.lstatSync(e);
  } catch (i) {
    return void t.visitFileFailed(e, i);
  }
  if (r.exclude && r.exclude.test(e)) return;
  if (r.filter && !r.filter.test(e)) return;
  const l = o.isFile() ? file_type_js_1.FileType.FILE : file_type_js_1.FileType.DIRECTORY,
    a = new FileInfo(PATH.basename(e), e, l, o.size, o.mtime.getTime(), s.isSymbolicLink());
  if (s.isSymbolicLink()) {
    if (false === r.followSymlinks) return;
    if (n.find(e => e === s.ino)) return;
    n.push(s.ino);
  }
  o.isFile() ? walkFile(e, a, t, r) : o.isDirectory() && walkDirectory(e, i, a, t, r, n);
}
export function walkFileTree(e, i, t = {}) {
  walkFileRecursive(e, 0, i, t, []);
}
