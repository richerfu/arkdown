/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/trace/json-trace.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as parse_json_util_js_1 from "../util/parse-json/parse-json-util";
export class JsonTrace {
  constructor(e) {
    const t = fs_1.readFileSync(path_1.resolve(e), {
      encoding: "utf-8"
    });
    try {
      this.telemetry = (0, parse_json_util_js_1.parseJsonText)(t);
    } catch (e) {
      this.telemetry = {};
    }
  }
  getRecord(e) {
    if (!Object.keys(this.telemetry).length) return [];
    const t = [];
    return this.collectRecordRecursively(e, t), t;
  }
  collectRecordRecursively(e, t, r = "") {
    if (isPlainObject(e)) for (const [s, o] of Object.entries(e)) {
      const e = r ? `${r}.${s}` : s;
      this.processCurrentNode(o, t, e), this.processChildNodes(o, t, e);
    }
  }
  processCurrentNode(e, t, r) {
    const s = this.telemetry[r];
    if (!(null == s ? undefined : s.trace)) return;
    const o = {
      path: r,
      traceValue: false,
      value: ""
    };
    (null == s ? undefined : s.traceValue) && (o.traceValue = true, o.value = e), t.push(o);
  }
  processChildNodes(e, t, r) {
    isObjectArray(e) ? e.forEach(e => this.collectRecordRecursively(e, t, r)) : this.collectRecordRecursively(e, t, r);
  }
}
function isPlainObject(e) {
  return "object" == typeof e && null !== e && !Array.isArray(e);
}
function isObjectArray(e) {
  return Array.isArray(e) && e.length > 0 && isPlainObject(e[0]);
}
