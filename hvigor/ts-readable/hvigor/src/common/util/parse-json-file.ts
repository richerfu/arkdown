/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/parse-json-file.js
 */
import fs_1 from "fs";
import os_1 from "os";
import path_1 from "path";
import * as param_replace_decorator_js_1 from "../../base/util/util-decorator/param-replace-decorator";
import * as hvigor_common_1 from "@ohos/hvigor-common";
var __decorate = this && this.__decorate || function (e, r, t, o) {
  var s,
    a = arguments.length,
    i = a < 3 ? r : null === o ? o = Object.getOwnPropertyDescriptor(r, t) : o;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) i = Reflect.decorate(e, r, t, o);else for (var n = e.length - 1; n >= 0; n--) (s = e[n]) && (i = (a < 3 ? s(i) : a > 3 ? s(r, t, i) : s(r, t)) || i);
  return a > 3 && i && Object.defineProperty(r, t, i), i;
};
export class ParseJsonFile {
  static parseJsonFile(e, r = false, t = "utf-8") {
    const o = fs_1.readFileSync(path_1.resolve(e), {
      encoding: t
    });
    try {
      return (0, hvigor_common_1.parseJsonText)(o, r);
    } catch (r) {
      if (r instanceof SyntaxError) {
        const t = r.message.split("at");
        if (2 === t.length) throw new Error(`${t[0].trim()}${os_1.EOL}\t at ${e}:${t[1].trim()}`);
      }
      throw new Error(`${e} is not in valid JSON/JSON5 format.`);
    }
  }
}
export function parseJsonFile(e, r = false, t = "utf-8") {
  return ParseJsonFile.parseJsonFile(e, r, t);
}
__decorate([(0, param_replace_decorator_js_1.WithParamReplacement)()], ParseJsonFile, "parseJsonFile", null);
