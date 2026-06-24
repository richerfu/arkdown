/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/json-util.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as ohos_logger_js_1 from "./log/ohos-logger";
var __decorate = this && this.__decorate || function (e, r, t, o) {
  var n,
    s = arguments.length,
    i = s < 3 ? r : null === o ? o = Object.getOwnPropertyDescriptor(r, t) : o;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) i = Reflect.decorate(e, r, t, o);else for (var _ = e.length - 1; _ >= 0; _--) (n = e[_]) && (i = (s < 3 ? n(i) : s > 3 ? n(r, t, i) : n(r, t)) || i);
  return s > 3 && i && Object.defineProperty(r, t, i), i;
};
const log = ohos_logger_js_1.OhosLogger.getLogger("json-util");
export class JsonUtil {
  static getJson5Obj(e, r = "utf-8") {
    if (fs_extra_1.existsSync(e)) try {
      const t = fs_extra_1.readFileSync(path_1.resolve(e), {
        encoding: r
      });
      return (0, hvigor_common_1.parseJsonText)(t);
    } catch (r) {
      if (r instanceof SyntaxError) {
        const t = r.message.split("at ");
        2 === t.length && log.printErrorExit("JSON_READER_SYNTAX_ERROR", [t[0].trim(), e, t[1].trim()]);
      }
      log.printErrorExit("NOT_CORRECT_JSON_FORMAT", [e]);
    }
  }
}
export function getJson5ObjThrowError(e, r = "utf-8") {
  if (!fs_extra_1.existsSync(e)) return;
  const t = fs_extra_1.readFileSync(path_1.resolve(e), {
    encoding: r
  });
  return (0, hvigor_common_1.parseJsonText)(t);
}
export function mergeStringJson(e, r) {
  const t = {
      string: []
    },
    o = [];
  return null == e || e.string.forEach(e => {
    t.string.push(e), o.push(e.name);
  }), null == r || r.string.forEach(e => {
    o.includes(e.name) || t.string.push(e);
  }), t;
}
__decorate([(0, hvigor_1.WithParamReplacement)()], JsonUtil, "getJson5Obj", null);
