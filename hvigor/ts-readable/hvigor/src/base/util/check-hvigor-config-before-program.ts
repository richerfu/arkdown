/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/check-hvigor-config-before-program.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as const_js_1 from "../../common/const/const";
import * as path_const_js_1 from "../../common/const/path-const";
import * as parse_json_file_js_1 from "../../common/util/parse-json-file";
export function checkHvigorConfigBeforeProgram(r, e) {
  try {
    return fs_1.existsSync(path_1.resolve(path_const_js_1.HVIGOR_PROJECT_WRAPPER_HOME, const_js_1.DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME)) ? (parse_json_file_js_1.ParseJsonFile.parseJsonFile(path_1.resolve(path_const_js_1.HVIGOR_PROJECT_WRAPPER_HOME, const_js_1.DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME)), {
      result: true
    }) : {
      result: true
    };
  } catch (o) {
    let t;
    return o instanceof Error ? (t = `${o.message}`, r.stacktrace && o.stack && (t += `${o.stack}\n`)) : t = "UnKnown Error", e.printError("SYNTAX_ERROR_IN_HVIGOR_CONFIG_JSON5", [t]), {
      result: false,
      stack: null == o ? undefined : o.stack,
      message: formatHvigorConfigErrorMsg(t)
    };
  }
}
function formatHvigorConfigErrorMsg(r) {
  return `00303236 Configuration Error\nError Message: ${r}\n\n* Try the following:\n  > Correct the syntax error as indicated above in the hvigor-config.json5 file.`;
}
