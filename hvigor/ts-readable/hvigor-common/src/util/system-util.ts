/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/system-util.js
 */
import os_1 from "os";
const WINDOWS_OS_NAME = "Windows_NT",
  LINUX_OS_NAME = "Linux",
  MAC_OS_NAME = "Darwin";
export function isWindows() {
  return "Windows_NT" === os_1.type();
}
export function isLinux() {
  return "Linux" === os_1.type();
}
export function isMac() {
  return "Darwin" === os_1.type();
}
export function maxPathLength() {
  return isMac() ? 1016 : isWindows() ? 259 : 4095;
}
