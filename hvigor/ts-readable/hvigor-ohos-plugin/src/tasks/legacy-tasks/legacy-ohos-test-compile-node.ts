/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-ohos-test-compile-node.js
 */
import fs_extra_1 from "fs-extra";
import * as task_names_js_1 from "../common/task-names";
import * as legacy_compile_node_js_1 from "./legacy-compile-node";
export class LegacyOhosTestCompileNode extends legacy_compile_node_js_1.LegacyCompileNode {
  constructor(e, s) {
    super(e, s, task_names_js_1.TaskNames.LegacyFATask.OHOS_TEST_COMPILE_NODE);
  }
  declareInputFiles() {
    const e = super.declareInputFiles();
    return this.mainCodePath && fs_extra_1.existsSync(this.mainCodePath) && e.addEntry(this.mainCodePath, {
      isDirectory: true
    }), e;
  }
}
