/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/cli/wrapper/prepare-node-path.js
 */
import fs_1 from "fs";
import * as path from "path";
import process_1 from "process";
import * as hvigor_log_js_1 from "../../base/log/hvigor-log";
import * as const_js_1 from "../../common/const/const";
import * as path_const_js_1 from "../../common/const/path-const";
import * as parse_json_file_js_1 from "../../common/util/parse-json-file";
import * as util_js_1 from "./util";
const _log = hvigor_log_js_1.HvigorLogger.getLogger("prepare-node-path");
function adaptNode14Module(e, t) {
  const o = require("module"),
    r = o._resolveFilename;
  o._resolveFilename = (o, s, n) => {
    var i;
    try {
      return r(o, s, n);
    } catch (_) {
      return null !== (i = function (o) {
        if (!t.dependencies[o]) return;
        const r = path.resolve(e, o);
        for (const e of [".js", ".ts"]) {
          const t = path.resolve(r, `index${e}`);
          if (fs_1.existsSync(t)) return _log.debug(`adaptNode14Module, request = ${o}, resolvedFile = ${t}`), t;
        }
      }(o)) && undefined !== i ? i : r(o, s, n);
    }
  };
}
export function initNodePath(e) {
  const t = e ? path.resolve(e, "node_modules") : path.resolve((0, util_js_1.calProjectCacheDir)(), "node_modules");
  process_1.env.NODE_PATH = const_js_1.DEFAULT_PROJECT_NODE_PATH ? t.concat(path.delimiter).concat(const_js_1.DEFAULT_PROJECT_NODE_PATH) : t, _log.debug(`env: nodejsVersion=${process_1.version}`);
  const o = readProjectHvigorConfig();
  return _log.debug("env: hvigor-config.json5 content =", o), process_1.version.startsWith("v14") && adaptNode14Module(t, o), require("module").Module._initPaths(), t;
}
function readProjectHvigorConfig() {
  const e = path.resolve(path_const_js_1.HVIGOR_PROJECT_WRAPPER_HOME, const_js_1.DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME);
  return fs_1.existsSync(e) ? parse_json_file_js_1.ParseJsonFile.parseJsonFile(e) : {
    properties: {
      "hvigor.dependency.useNpm": true
    },
    modelVersion: "",
    dependencies: {}
  };
}
