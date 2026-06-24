/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/cli/util/std-hook.js
 */
import * as wdk_1 from "@baize/wdk";
import * as time_util_1 from "../../base/util/time-util";
export class StdHook {
  constructor(o, t) {
    this.promise = o, this.unhook = t;
  }
}
const hook = (o, t) => {
  const {
    write: r
  } = o;
  let e = wdk_1.noop;
  return new StdHook(new Promise(i => {
    e = () => {
      o.write = r, i();
    }, o.write = o => t(String(o));
  }), e);
};
function stdOutHook(o) {
  return hook(process.stdout, o);
}
function stdErrHook(o) {
  return hook(process.stderr, o);
}
const timePrefixLength = 26,
  deprecationWarning = "DeprecationWarning:";
let outHook, errHook;
function getOutHook() {
  return stdOutHook(o => (outHook.unhook(), write(o, true), outHook = getOutHook(), true));
}
function getErrHook() {
  return stdErrHook(o => (errHook.unhook(), write(o), errHook = getErrHook(), true));
}
function write(o, t) {
  const r = t ? process.stdout : process.stderr;
  o.includes(deprecationWarning) || ((0, time_util_1.isHvigorLogWithTime)(o) ? r.write(o.substring(26)) : r.write(o));
}
export function initHook() {
  outHook = getOutHook(), errHook = getErrHook();
}
export function unhook() {
  outHook.unhook(), errHook.unhook();
}
