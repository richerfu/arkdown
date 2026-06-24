/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/core-process-package-json.js
 */
import * as pre_build_js_1 from "../pre-build";
import * as ohos_har_task_js_1 from "../task/ohos-har-task";
export class CoreProcessPackageJson extends ohos_har_task_js_1.OhosHarTask {
  constructor(e, s) {
    super(e, s), this.generatePmDir = this.pathInfo.getGeneratePmDir();
  }
  doTaskAction() {
    (this._harExtendInfo.isObfuscatedHar() || this.targetService.isByteCodeHar()) && this.getGeneratePackageJsonObj();
  }
  getGeneratePackageJsonObj() {
    const e = this.getPackageJsonValidMainField();
    packageJsonValidMainField = e;
    const s = this.getDefaultTypesValueByMainField(e),
      a = this.packageJsonObj;
    return a.main && a.main.endsWith(".js") || this.targetService.isCustomizedHar() && !a.main || (a.types = s), a.main = this.replaceMainFieldSuffix(e), this.addArtifactType(a), a;
  }
  getDefaultTypesValueByMainField(e) {
    let s;
    return ohos_har_task_js_1.DEFAULT_MAIN_FILED_FILE_SUFFIX.forEach(a => {
      e.endsWith(a) && (s = e.replace(a, `.d${a}`));
    }), null != s ? s : "index.d.ets";
  }
  initTaskDepends() {
    this.declareDepends(pre_build_js_1.PreBuild.name);
  }
  replaceMainFieldSuffix(e) {
    const s = this.hasUseTsHarField();
    for (const a of ohos_har_task_js_1.DEFAULT_MAIN_FILED_FILE_SUFFIX) {
      if (s) return e.replace(a, ".ts");
      if (e.endsWith(a)) return e.replace(a, ".js");
    }
    return e.concat(".js");
  }
}
export { packageJsonValidMainField };
