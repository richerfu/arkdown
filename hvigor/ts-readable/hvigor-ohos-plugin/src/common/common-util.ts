/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/common-util.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as common_const_js_1 from "../const/common-const";
export function isSingleFramework() {
  const e = path_1.resolve(__dirname, "../../res", common_const_js_1.CommonConst.FRAMEWORK_CONFIGURATION);
  if (fs_extra_1.existsSync(e)) {
    return hvigor_1.Json5Reader.getJson5Obj(e).frameworkType !== common_const_js_1.CommonConst.DOUBLE_FRAMEWORK;
  }
  return true;
}
export function parsingProfileName(e) {
  if (/^[$]profile:[0-9a-zA-Z_.]+|(?=.*[{])(?=.*[}])[0-9a-zA-Z_.{}]+$/.test(e)) return e.split(":")[1];
}
function getSingleWord(e, r, t, o) {
  var i;
  const n = e[0],
    s = null === (i = e[1]) || undefined === i ? undefined : i.trim();
  if (!s) return void o.push(n);
  let a = r.get(s);
  if (undefined === a) {
    const e = s.split(".");
    let o = t;
    for (let r = 0; r < e.length; r++) {
      if (null === o || "object" != typeof o) {
        o = undefined;
        break;
      }
      o = o[e[r]];
    }
    a = null !== o && "object" != typeof o && undefined !== o ? String(o) : n, r.set(s, a);
  }
  o.push(a);
}
export function replaceStringParam(e, r) {
  if (!e || !r) return e;
  if (-1 === e.indexOf("${")) return e;
  const t = [],
    o = new Map(),
    i = /\$\{([^}]+)\}/g;
  let n,
    s = 0,
    a = false;
  for (; null !== (n = i.exec(e));) a = true, t.push(e.substring(s, n.index)), getSingleWord(n, o, r, t), s = i.lastIndex;
  return a ? (t.push(e.substring(s)), t.join("")) : e;
}
