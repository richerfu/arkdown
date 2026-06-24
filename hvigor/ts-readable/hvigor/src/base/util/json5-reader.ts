/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/json5-reader.js
 */
import * as fs from "fs";
import * as promises_1 from "fs/promises";
import * as path from "path";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as hvigor_log_js_1 from "../log/hvigor-log";
import * as param_replace_decorator_js_1 from "./util-decorator/param-replace-decorator";
var __decorate = this && this.__decorate || function (e, r, t, o) {
  var n,
    i = arguments.length,
    a = i < 3 ? r : null === o ? o = Object.getOwnPropertyDescriptor(r, t) : o;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) a = Reflect.decorate(e, r, t, o);else for (var s = e.length - 1; s >= 0; s--) (n = e[s]) && (a = (i < 3 ? n(a) : i > 3 ? n(r, t, a) : n(r, t)) || a);
  return i > 3 && a && Object.defineProperty(r, t, a), a;
};
export class Json5Reader {
  static getJson5Obj(e, r = "utf-8") {
    fs.existsSync(e) || Json5Reader.logger.printErrorExit("FILE_NOT_EXIST", [e]);
    const t = fs.readFileSync(path.resolve(e), {
      encoding: r
    });
    try {
      return (0, hvigor_common_1.parseJsonText)(t);
    } catch (r) {
      Json5Reader.handleException(e, r);
    }
  }
  static async readJson5File(e, r = "utf-8") {
    try {
      return (0, promises_1.readFile)(e, {
        encoding: r
      }).then(hvigor_common_1.parseJsonText);
    } catch (r) {
      Json5Reader.handleException(e, r);
    }
  }
  static handleException(e, r) {
    if (r instanceof SyntaxError) {
      const t = r.message.split("at ");
      2 === t.length && Json5Reader.logger.printErrorExit("JSON_READER_SYNTAX_ERROR", [t[0].trim(), e, t[1].trim()]);
    }
    Json5Reader.logger.printErrorExit("NOT_CORRECT_JSON_FORMAT", [e]);
  }
  static getJson5ObjProp(e, r) {
    const t = r.split(".");
    let o = e;
    for (const e of t) {
      if (undefined === o[e]) return;
      o = o[e];
    }
    return o;
  }
}
Json5Reader.logger = hvigor_log_js_1.HvigorLogger.getLogger(Json5Reader.name), __decorate([(0, param_replace_decorator_js_1.WithParamReplacement)()], Json5Reader, "getJson5Obj", null);
