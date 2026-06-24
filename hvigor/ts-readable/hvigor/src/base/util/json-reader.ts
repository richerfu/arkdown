/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/json-reader.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as hvigor_log_js_1 from "../log/hvigor-log";
export class JsonReader {
  static getJsonText(e, r = "utf-8") {
    return fs_1.readFileSync(path_1.resolve(e), {
      encoding: r
    });
  }
  static getJsonObj(e, r = "utf-8") {
    const t = fs_1.readFileSync(path_1.resolve(e), {
      encoding: r
    });
    try {
      return JSON.parse(t);
    } catch (r) {
      if (r instanceof SyntaxError) {
        const t = r.message.split("at ");
        2 === t.length && JsonReader.logger.printErrorExit("JSON_READER_SYNTAX_ERROR", [t[0].trim(), e, t[1].trim()]);
      }
      JsonReader.logger.printErrorExit("NOT_CORRECT_JSON_FORMAT", [e]);
    }
  }
  static parseJsonText(e, r, t) {
    let o;
    try {
      o = JSON.parse(e, t);
    } catch (t) {
      o = r, JsonReader.logger.warn(`Failed to parse the JSON text '${e}'.`);
    }
    return o;
  }
}
JsonReader.logger = hvigor_log_js_1.HvigorLogger.getLogger(JsonReader.name);
