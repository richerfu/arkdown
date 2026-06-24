/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/form-util.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as file_util_js_1 from "./file-util";
import * as json_util_js_1 from "./json-util";
export class FormUtil {
  static getFormConfigPathInModuleRes(e, t) {
    const o = e.getSourceSetByTargetName(t).getTargetResPath(),
      r = path_1.resolve(o, "base", "profile"),
      l = file_util_js_1.FileUtil.getAllFilesFromFolder(r);
    for (const e of l) {
      const t = json_util_js_1.JsonUtil.getJson5Obj(e);
      if (null == t ? undefined : t.forms) return e;
    }
  }
  static resolveFormConfigPath(e, t, o) {
    const r = e.getSourceSetByTargetName(t).getTargetResPath();
    return path_1.resolve(r, o);
  }
  static getFormWidgetModuleByTargetName(e, t) {
    if (e.isFormExtensionModule(t)) {
      const o = e.getSourceSetByTargetName(t).getModuleTargetRes().getModuleJsonOpt().module.formWidgetModule;
      return e.getParentProject().getModuleModelByName(null != o ? o : "");
    }
  }
  static getFormExtensionModuleByTargetName(e, t) {
    if (e.isFormWidgetModule(t)) {
      const o = e.getSourceSetByTargetName(t).getModuleTargetRes().getModuleJsonOpt().module.formExtensionModule;
      return e.getParentProject().getModuleModelByName(null != o ? o : "");
    }
  }
  static getTargetModuleOptObj(e, t) {
    const o = e.getModuleService().getModuleModel(),
      r = e.getTargetData().getTargetName();
    let l = t.getModuleJsonOpt();
    const i = FormUtil.getFormExtensionModuleByTargetName(o, r);
    if (!i) return l;
    l = (0, wdk_1.cloneDeep)(l);
    const s = i.getJsonObjByTargetName(r);
    return s.module.extensionAbilities ? (s.module.extensionAbilities.filter(e => "form" === e.type).forEach(e => {
      l.module.extensionAbilities || (l.module.extensionAbilities = []);
      const t = {
        name: e.name,
        type: e.type,
        metadata: e.metadata
      };
      l.module.extensionAbilities.push(t);
    }), l) : l;
  }
}
