/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/resolve/lib/homedir.js
 */
import os_1 from "os";
const home = process.env.HOME,
  user = process.env.LOGNAME || process.env.USER || process.env.LNAME || process.env.USERNAME;
function win32HomeDir() {
  let e;
  return process.env.HOMEDRIVE && process.env.HOMEPATH && (e = process.env.HOMEDRIVE + process.env.HOMEPATH), process.env.USERPROFILE || e || home || null;
}
function darwinHomeDir() {
  return home || (user ? `/Users/${user}` : null);
}
function linuxHomeDir() {
  return home || (0 === process.getuid() ? "/root" : user ? `/home/${user}` : null);
}
export const getHomedir = os_1.homedir || function () {
  return "win32" === process.platform ? win32HomeDir() : "darwin" === process.platform ? darwinHomeDir() : "linux" === process.platform ? linuxHomeDir() : home || null;
};
