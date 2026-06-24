/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/model/cxx/generate-codemodel.js
 */
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as json_util_js_1 from "../../utils/json-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
export class GenerateCodemodel {
  constructor(e) {
    this._log = ohos_logger_js_1.OhosLogger.getLogger(GenerateCodemodel.name), this.reply = path_1.resolve(e, ".cmake", "api", "v1", "reply"), this.index = this.readIndex(), this._codemodel = this.generateCodemodel();
  }
  getCodemodel() {
    return this._codemodel;
  }
  getObjFromIndex(e) {
    return Object.values(this.index.reply).filter(t => t.kind === e).map(e => json_util_js_1.JsonUtil.getJson5Obj(path_1.resolve(this.reply, e.jsonFile))).shift();
  }
  generateCodemodel() {
    const e = this.readCodemodel(this.index.reply);
    return this.readTarget(e), e;
  }
  readCodemodel(e) {
    if (!e || !e["codemodel-v2"]) return void this._log.warn("Failed to generate CMake code mode. The native code will skip compiling.");
    const t = e["codemodel-v2"],
      o = null == t ? undefined : t.jsonFile;
    if (!o) return;
    const r = path_1.resolve(this.reply, o);
    return json_util_js_1.JsonUtil.getJson5Obj(r);
  }
  readIndex() {
    fs_extra_1.existsSync(this.reply) || this._log.printErrorExit("CMAKE_TASK_EXECUTION_FAILED");
    const e = fs_extra_1.readdirSync(this.reply).find(e => e.startsWith("index-"));
    return e || this._log.printErrorExit("CMAKE_FILE_API_PARSE_FAILED", [this.reply], [[this.reply]]), json_util_js_1.JsonUtil.getJson5Obj(path_1.resolve(this.reply, e));
  }
  readTarget(e) {
    var t;
    null === (t = null == e ? undefined : e.configurations) || undefined === t || t.forEach(e => {
      var t;
      if (e.targets) for (let o = 0; o < (null === (t = e.targets) || undefined === t ? undefined : t.length); o++) {
        const t = e.targets[o].jsonFile;
        if (!t) return;
        e.targets[o] = json_util_js_1.JsonUtil.getJson5Obj(path_1.resolve(this.reply, t));
      }
    });
  }
}
