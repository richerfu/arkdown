/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/build-profile-utils.js
 */
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as array_util_js_1 from "./array-util";
import * as common_const_js_1 from "../const/common-const";
export const normalizedFileData = (e, t, s) => `${(0, generateConstVariable)(e, t, s)}\n${(0, generateExportClass)(e)}`;
export const generateConstVariable = (e, t, s) => {
  const o = "export const ";
  let r = "/**\n * Use these variables when you tailor your ArkTS code. They must be of the const type.\n */\n";
  return Object.keys(e).forEach(a => {
    const n = e[a];
    "string" == typeof n ? r += `${o + a} = '${n}';\n` : "number" == typeof n || "boolean" == typeof n ? r += `${o + a} = ${n};\n` : s.printErrorExit("BUILDPROFILEFIELDS_ONLY_SUPPORT_NUMBER_STRING_BOOLEAN", [typeof n, n, t.getModuleModel().getProfilePath()]);
  }), r;
};
export const generateExportClass = e => {
  let t = "/**\n * BuildProfile Class is used only for compatibility purposes.\n */\nexport default class BuildProfile { \n";
  return Object.keys(e).forEach(e => {
    t += `\t${"static readonly " + e} = ${e};\n`;
  }), t += "}", t;
};
export const getRuntimeOnlyObjMap = (e, t, s, o) => {
  const r = new Map();
  return t ? (t.sources && t.sources.forEach(t => {
    const s = path_1.resolve(e, t);
    fs_extra_1.existsSync(s) && r.set(t, s);
  }), t.packages && (0, array_util_js_1.isSubset)(s, t.packages) && r.set(`runtimeOnlyPackages${null != o ? o : ""}`, t.packages), t.excludePackages && r.set(`runtimeOnlyExcludePackages${null != o ? o : ""}`, t.excludePackages), r) : r;
};
export const getCustomTypePathIfExists = (e, t) => {
  const s = path_1.isAbsolute(t) ? t : path_1.resolve(e, t);
  for (const e of common_const_js_1.CustomTypesConst.CUSTOM_TYPES_SUFFIX) {
    const t = `${s}${e}`;
    if (fs_extra_1.existsSync(`${s}${e}`)) return t;
  }
  for (const e of common_const_js_1.CustomTypesConst.CUSTOM_TYPES_INDEX) {
    const t = path_1.resolve(s, e);
    if (fs_extra_1.existsSync(t)) return t;
  }
};
