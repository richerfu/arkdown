/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/generator/default-task-snapshot-generator.js
 */
import path_1 from "path";
import * as incremental_exec_task_js_1 from "../../../external/task/incremental-exec-task";
import * as json_reader_js_1 from "../../../util/json-reader";
import * as default_task_snapshot_js_1 from "../core/default-task-snapshot";
import * as task_built_in_js_1 from "../util/task-built-in";
var Execution = task_built_in_js_1.TaskBuiltIn.Execution,
  snapshotSerializationReplacer = task_built_in_js_1.TaskBuiltIn.snapshotSerializationReplacer,
  snapshotSerializationReceiver = task_built_in_js_1.TaskBuiltIn.snapshotSerializationReceiver;
export class DefaultTaskSnapshotGenerator {
  snapshot(e) {
    const t = new default_task_snapshot_js_1.DefaultTaskSnapshot(e);
    if (e instanceof incremental_exec_task_js_1.IncrementalExecTask) {
      t.addInput(Execution.COMMAND, e.declareExecutionCommand()), t.addInput(Execution.TOOLCHAIN, e.declareExecutionTool());
      const a = e.declareExecutionEnv(),
        s = [];
      Array.from(a.keys()).sort().forEach(e => s.push(`${e}:${a.get(e)}${path_1.delimiter}`)), t.addInput(Execution.ENV, s.join());
    }
    return t;
  }
  loadSnapshotFromJson(e) {
    const t = json_reader_js_1.JsonReader.parseJsonText(e, {}, snapshotSerializationReceiver);
    return Object.setPrototypeOf(t, default_task_snapshot_js_1.DefaultTaskSnapshot.prototype), t;
  }
  loadSnapshotCacheFromJson(e) {
    const t = json_reader_js_1.JsonReader.parseJsonText(e, new Map(), snapshotSerializationReceiver),
      a = new Map();
    for (const [e, s] of Object.entries(t)) a.set(e, Object.setPrototypeOf(s, default_task_snapshot_js_1.DefaultTaskSnapshot.prototype));
    return a;
  }
  serializeSnapshotCacheToJson(e) {
    const t = Object.fromEntries(e.entries());
    return JSON.stringify(t, snapshotSerializationReplacer);
  }
}
