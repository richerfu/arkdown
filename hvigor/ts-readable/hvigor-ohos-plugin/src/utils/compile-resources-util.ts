/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/compile-resources-util.js
 */
import * as hap_extra_info_js_1 from "../project/data/hap-extra-info";
export class CompileResourcesUtil {
  static getCompileTargetFolderBySeq(e, o) {
    return `${e}${0 === o ? "" : o}`;
  }
  static getRestoolModuleNames(e, o) {
    var t, s;
    const l = e.getModuleModel(),
      r = e.getProjectModel(),
      i = [l.getModule().getName()];
    return l.getApiType() === hap_extra_info_js_1.ApiType.FA ? i.push(null === (t = l.getSourceSetByTargetName(o).getLegacyModuleTargetRes().getConfigJsonOpt()) || undefined === t ? undefined : t.module.distro.moduleName) : i.push(null === (s = l.getSourceSetByTargetName(o).getModuleTargetRes().getModuleJsonOpt()) || undefined === s ? undefined : s.module.name), [...new Set(i.concat(...r.getProject().getSubModules().keys()))];
  }
}
export var CompileResourceBuildCommandType;
(function (e) {
  e.FILE = "file", e.COMMAND = "command";
})(CompileResourceBuildCommandType = CompileResourceBuildCommandType || (CompileResourceBuildCommandType = {}));
