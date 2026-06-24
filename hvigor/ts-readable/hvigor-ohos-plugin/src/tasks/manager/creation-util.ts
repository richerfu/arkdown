/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/manager/creation-util.js
 */
import * as common_const_js_1 from "../../const/common-const";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as sign_hap_js_1 from "../sign-hap";
export const computeTaskDetailsWithTarget = (e, t) => ({
  ...e,
  name: `${t.getTargetData().getTargetName()}@${e.name}`
});
export const computeDepends = (e, t) => {
  const s = t.getTargetData().getTargetName(),
    o = t.getModuleService().getModuleModel().getModule();
  return e.map(e => e.includes(":") || e.includes("@") ? e : `${o.getName()}:${s}@${e}`);
};
export const deviceTestComputeDepends = (e, t, s, o) => {
  if (t.includes(":") || t.includes("@")) return void e.push(t);
  const p = s.getTargetData().getTargetName(),
    u = s.getModuleService().getModuleModel().getModule(),
    n = `${u.getName()}:${p}@${t}`,
    a = `${u.getName()}:${common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET}@${o === module_type_enum_js_1.ModuleType.Entry || o === module_type_enum_js_1.ModuleType.Feature ? t : sign_hap_js_1.SignHap.name}`;
  e.push(n, a);
};
export const computeHookTaskDepends = (e, t) => {
  const s = [];
  return t.forEach(t => s.push(...(0, computeDepends)(e, t))), s;
};
