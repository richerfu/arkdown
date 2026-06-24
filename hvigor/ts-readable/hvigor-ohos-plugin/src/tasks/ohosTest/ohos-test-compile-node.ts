/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/ohosTest/ohos-test-compile-node.js
 */
import * as fse from "fs-extra";
import * as task_names_js_1 from "../common/task-names";
import * as compile_node_js_1 from "../compile-node";
var Task = task_names_js_1.TaskNames.Task;
export class OhosTestCompileNode extends compile_node_js_1.CompileNode {
  constructor(e, t) {
    super(e, t, Task.OHOS_TEST_COMPILE_NODE);
  }
  declareInputFiles() {
    const e = super.declareInputFiles();
    return this.mainCodePath && fse.existsSync(this.mainCodePath) && e.addEntry(this.mainCodePath, {
      isDirectory: true
    }), e;
  }
}
