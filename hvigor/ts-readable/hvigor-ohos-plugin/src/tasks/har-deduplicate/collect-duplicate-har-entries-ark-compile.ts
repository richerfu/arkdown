/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har-deduplicate/collect-duplicate-har-entries-ark-compile.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as collect_duplicate_har_dependencies_js_1 from "../../har-dedepulicate/collect_duplicate_har_dependencies";
import * as ark_compile_js_1 from "../ark-compile";
var __decorate = this && this.__decorate || function (e, t, r, i) {
  var a,
    o = arguments.length,
    p = o < 3 ? t : null === i ? i = Object.getOwnPropertyDescriptor(t, r) : i;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) p = Reflect.decorate(e, t, r, i);else for (var l = e.length - 1; l >= 0; l--) (a = e[l]) && (p = (o < 3 ? a(p) : o > 3 ? a(t, r, p) : a(t, r)) || p);
  return o > 3 && p && Object.defineProperty(t, r, p), p;
};
export class CollectDuplicateHarEntriesArkCompile extends ark_compile_js_1.ArkCompile {
  constructor(e, t, r) {
    super(e, t, r), this.hspNeedDeduplicateHarInfoMap = (0, collect_duplicate_har_dependencies_js_1.getHspNeedDeduplicateHarInfoMap)(this.moduleName);
  }
  async initDefaultArkCompileConfig() {
    const e = await super.initDefaultArkCompileConfig();
    return e.isHarDeduplicateArkCompile = true, this.hspNeedDeduplicateHarInfoMap && Object.keys(this.hspNeedDeduplicateHarInfoMap).length > 0 && (e.hspDeduplicateHarInfo = this.hspNeedDeduplicateHarInfoMap), e;
  }
  taskShouldDo() {
    return this.targetService.shouldDoDeduplicateTask() && undefined !== this.hspNeedDeduplicateHarInfoMap && Object.keys(this.hspNeedDeduplicateHarInfoMap).length > 0;
  }
  declareOutputFiles() {
    const e = new hvigor_1.FileSet();
    if (this.hspNeedDeduplicateHarInfoMap) {
      let t = false,
        r = false;
      for (const e of Object.values(this.hspNeedDeduplicateHarInfoMap)) if (e.isByteCodeHarDependency ? t = true : e.isHarDependency && (r = true), t && r) break;
      t && e.addEntry(this.pathInfo.getByteCodeHarDuplicateFilesInfoPath()), r && e.addEntry(this.pathInfo.getSourceHarDuplicateFilesInfoPath());
    }
    return e;
  }
}
__decorate([(0, hvigor_1.Input)()], CollectDuplicateHarEntriesArkCompile.prototype, "hspNeedDeduplicateHarInfoMap", undefined);
