/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/util/task-built-in.js
 */
import * as json_reader_js_1 from "../../../util/json-reader";
import * as task_input_value_entry_js_1 from "./task-input-value-entry";
export var TaskBuiltIn;
!function (e) {
  let t, a;
  !function (e) {
    e.COMMAND = "BUILTIN_TASK_COMMAND", e.TOOLCHAIN = "BUILTIN_TASK_TOOLCHAIN", e.ENV = "BUILTIN_TASK_ENV";
  }(t = e.Execution || (e.Execution = {})), function (e) {
    e.MAP = "Map", e.VALUE_ENTRY = "ValueEntry", e.REG_EXP = "RegExp";
  }(a || (a = {}));
  const n = new Map();
  n.set(a.MAP, e => new Map(e.value)), n.set(a.VALUE_ENTRY, e => {
    const t = json_reader_js_1.JsonReader.parseJsonText(e.value, undefined);
    if (t) return new task_input_value_entry_js_1.ValueEntry(t._name, t._value);
  }), n.set(a.REG_EXP, e => new RegExp(e.value));
  const s = new Map();
  s.set(Map, e => ({
    dataType: a.MAP,
    value: Array.from(e.entries())
  })), s.set(task_input_value_entry_js_1.ValueEntry, e => ({
    dataType: a.VALUE_ENTRY,
    value: JSON.stringify(e)
  })), s.set(RegExp, e => ({
    dataType: a.REG_EXP,
    value: e.source
  })), e.snapshotSerializationReplacer = (e, t) => {
    const a = [Map, task_input_value_entry_js_1.ValueEntry, RegExp];
    for (const e of a) if (t instanceof e && s.has(e)) return s.get(e)(t);
    return t;
  }, e.snapshotSerializationReceiver = (e, t) => "object" == typeof t && null !== t && n.has(t.dataType) ? n.get(t.dataType)(t) : t;
}(TaskBuiltIn = TaskBuiltIn || (TaskBuiltIn = {}));
