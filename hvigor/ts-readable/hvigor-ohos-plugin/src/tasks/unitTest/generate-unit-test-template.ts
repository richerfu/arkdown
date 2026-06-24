/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/unitTest/generate-unit-test-template.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import * as common_const_1 from "../../const/common-const";
import * as task_names_js_1 from "../common/task-names";
import * as ohos_module_task_js_1 from "../task/ohos-module-task";
export class GenerateUnitTestTemplate extends ohos_module_task_js_1.OhosModuleTask {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.Task.GENERATE_UNIT_TEST_TEMPLATE);
  }
  getParams() {
    const e = hvigor_1.hvigorCore.getExtraConfig();
    return {
      bundleName: this.projectModel.getAppRes().getAppResOpt().app.bundleName,
      moduleName: this.moduleName,
      scope: e.get(common_const_1.CommonConst.OHOS_TEST_COVERAGE_SCOPE),
      coverage: "true" === e.get(common_const_1.CommonConst.OHOS_TEST_COVERAGE),
      modulePath: this.pathInfo.getModulePath()
    };
  }
  declareOutputFiles() {
    const e = super.declareOutputFiles();
    return e.addEntry(this.pathInfo.getUnitTestTemplatePath(), {
      isDirectory: true
    }), e;
  }
  declareInputs() {
    const e = super.declareInputs(),
      t = this.getParams();
    return Object.keys(t).forEach(s => {
      const o = t[s];
      undefined !== o && e.set(s, o);
    }), e;
  }
  async doTaskAction() {
    (0, hvigor_arkts_compose_1.generateUnitTestTemplate)(this.getParams());
  }
  initTaskDepends() {}
}
