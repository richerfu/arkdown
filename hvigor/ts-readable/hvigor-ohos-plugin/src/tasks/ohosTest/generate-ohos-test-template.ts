/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/ohosTest/generate-ohos-test-template.js
 */
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import * as task_names_js_1 from "../common/task-names";
import * as ohos_module_task_js_1 from "../task/ohos-module-task";
export class GenerateOhosTestTemplate extends ohos_module_task_js_1.OhosModuleTask {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.Task.GENERATE_OHOS_TEST_TEMPLATE);
  }
  getParams() {
    return {
      sourceRoot: this.pathInfo.getModuleTestIntermediatesPath(),
      isCrossPlatform: this.projectModel.isCrossplatformProject(),
      apiVersion: this.compileApiVersion
    };
  }
  declareOutputFiles() {
    const e = super.declareOutputFiles();
    return e.addEntry(this.pathInfo.getModuleTestIntermediatesSrcOhosTestPath(), {
      isDirectory: true
    }), e;
  }
  declareInputs() {
    const e = super.declareInputs(),
      s = this.getParams();
    return Object.keys(s).forEach(t => {
      const o = s[t];
      undefined !== o && e.set(t.toString(), o);
    }), e;
  }
  async doTaskAction() {
    (0, hvigor_arkts_compose_1.generateOhosTestTemplate)(this.getParams());
  }
  initTaskDepends() {}
}
