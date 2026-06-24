/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/task/test-copy-mock-config-json.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as ohos_hap_task_js_1 from "./ohos-hap-task";
var __decorate = this && this.__decorate || function (o, e, t, s) {
  var i,
    r = arguments.length,
    n = r < 3 ? e : null === s ? s = Object.getOwnPropertyDescriptor(e, t) : s;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) n = Reflect.decorate(o, e, t, s);else for (var c = o.length - 1; c >= 0; c--) (i = o[c]) && (n = (r < 3 ? i(n) : r > 3 ? i(e, t, n) : i(e, t)) || n);
  return r > 3 && n && Object.defineProperty(e, t, n), n;
};
export class TestCopyMockConfigJson extends ohos_hap_task_js_1.OhosHapTask {
  constructor() {
    super(...arguments), this._log = ohos_logger_js_1.OhosLogger.getLogger(TestCopyMockConfigJson.name);
  }
  get mockConfigJsonFile() {
    return this.pathInfo.getIntermediatesMockConfigJsonFile();
  }
  get mockConfigJsonOutputFile() {
    return path_1.join(this.pathInfo.getIntermediatesResourcesRawFileMockDir(), build_directory_const_js_1.BuildArtifactConst.MOCK_CONFIG_JSON);
  }
  doTaskAction() {
    this.copyMockConfigJson();
  }
  copyMockConfigJson() {
    if (fs_extra_1.existsSync(this.mockConfigJsonFile)) {
      const o = this.pathInfo.getIntermediatesResourcesRawFileMockDir();
      fs_extra_1.ensureDirSync(o), fs_extra_1.copyFileSync(this.mockConfigJsonFile, this.mockConfigJsonOutputFile);
    } else this._log.debug("Failed to generate mock-config.json during compilation.");
  }
}
__decorate([(0, hvigor_1.InputFile)()], TestCopyMockConfigJson.prototype, "mockConfigJsonFile", null), __decorate([(0, hvigor_1.OutputFile)()], TestCopyMockConfigJson.prototype, "mockConfigJsonOutputFile", null);
