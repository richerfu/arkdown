/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-generate-metadata.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as common_const_js_1 from "../../const/common-const";
import * as abstract_generate_metadata_js_1 from "../abstract/abstract-generate-metadata";
import * as task_names_js_1 from "../common/task-names";
var __decorate = this && this.__decorate || function (e, t, a, r) {
  var o,
    s = arguments.length,
    n = s < 3 ? t : null === r ? r = Object.getOwnPropertyDescriptor(t, a) : r;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) n = Reflect.decorate(e, t, a, r);else for (var c = e.length - 1; c >= 0; c--) (o = e[c]) && (n = (s < 3 ? o(n) : s > 3 ? o(t, a, n) : o(t, a)) || n);
  return s > 3 && n && Object.defineProperty(t, a, n), n;
};
var LegacyFATask = task_names_js_1.TaskNames.LegacyFATask;
export class LegacyGenerateMetadata extends abstract_generate_metadata_js_1.AbstractGenerateMetadata {
  get relatedEntryJsonList() {
    return this.getRelatedEntryJsonList(common_const_js_1.CommonConst.CONFIG_JSON);
  }
  constructor(e) {
    super(e, LegacyFATask.GENERATE_METADATA);
    const t = this.sourceSetModel;
    this.configJsonPath = t.getLegacyModuleTargetRes().getJsonPath();
  }
  getDistroFilterObj(e) {
    var t;
    const a = e.getModuleSourceSetModel(),
      r = path_1.resolve(a.getLegacyModuleTargetRes().getJsonPath()),
      o = fs_extra_1.readJsonSync(r).module.distroFilter,
      s = null === (t = e.getTargetOpt().config) || undefined === t ? undefined : t.distroFilter;
    return null != s ? s : o;
  }
}
__decorate([(0, hvigor_1.InputFile)()], LegacyGenerateMetadata.prototype, "configJsonPath", undefined), __decorate([(0, hvigor_1.InputFiles)()], LegacyGenerateMetadata.prototype, "relatedEntryJsonList", null);
