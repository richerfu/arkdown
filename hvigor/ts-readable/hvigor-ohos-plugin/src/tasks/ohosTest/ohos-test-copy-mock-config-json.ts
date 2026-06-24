/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/ohosTest/ohos-test-copy-mock-config-json.js
 */
import * as task_names_js_1 from "../common/task-names";
import * as test_copy_mock_config_json_js_1 from "../task/test-copy-mock-config-json";
var Task = task_names_js_1.TaskNames.Task;
export class OhosTestCopyMockConfigJson extends test_copy_mock_config_json_js_1.TestCopyMockConfigJson {
  constructor(s) {
    super(s, Task.OHOS_TEST_COPY_MOCK_CONFIG_JSON);
  }
  initTaskDepends() {
    this.declareDepends("OhosTestCompileArkTS");
  }
}
