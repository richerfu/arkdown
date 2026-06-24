/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/fs.js
 */
import fs_1 from "fs";
import os_1 from "os";
import path_1 from "path";
import * as util_1 from "util";
export const access = (0, util_1.promisify)(fs_1.access);
export const readFile = (0, util_1.promisify)(fs_1.readFile);
export const stat = (0, util_1.promisify)(fs_1.stat);
const realpath = (0, util_1.promisify)(fs_1.realpath && "function" == typeof fs_1.realpath.native ? fs_1.realpath.native : fs_1.realpath),
  realpathCache = new Map(),
  fileCache = new Map();
let useRealpathCache = false;
export function clearRealpathCache() {
  realpathCache.clear();
}
export function clearFileCache() {
  fileCache.clear();
}
export function enableRealpathCache() {
  useRealpathCache = true;
}
export function disableRealpathCache() {
  useRealpathCache = false;
}
export async function fileExists(e) {
  try {
    return (await (0, stat)(e)).isFile();
  } catch {
    return false;
  }
}
export async function checkFileExistsAndSize(e) {
  try {
    let a;
    return fileCache.has(e) ? a = fileCache.get(e) : (a = await (0, stat)(e), fileCache.set(e, a)), a && a.isFile() && a.size > 10485760;
  } catch {
    return false;
  }
}
export async function resolveSymlink(e) {
  return (await fileExists(e)) ? realpathFS(e) : e;
}
export async function realpathFS(e) {
  if (useRealpathCache) {
    if (!realpathCache.has(e)) {
      const a = realpath(e);
      realpathCache.set(e, a);
    }
    return realpathCache.get(e);
  }
  return realpath(e);
}
export function transformPath(e) {
  return "Windows_NT" === os_1.type() && process.version < "v16.19.0" ? path_1.toNamespacedPath(e) : e;
}
