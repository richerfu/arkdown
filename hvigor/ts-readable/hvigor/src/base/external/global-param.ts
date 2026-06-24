/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/global-param.js
 */
import * as child_process_1 from "child_process";
import fs_1 from "fs";
import os_1 from "os";
import path_1 from "path";
import * as hvigor_log_js_1 from "../log/hvigor-log";
export class GlobalParam {
  static getInstance() {
    return this.instance || (this.instance = new GlobalParam()), this.instance;
  }
  constructor() {
    this._log = hvigor_log_js_1.HvigorLogger.getLogger("Init global params"), this.configuration = new Map(), this._initialized = false;
  }
  load() {
    const t = "Windows_NT" === os_1.type() ? "npm.cmd" : "npm",
      e = path_1.resolve(path_1.dirname(process.execPath), t);
    if (!fs_1.existsSync(e)) return void this._log.debug("Npm not found.");
    const i = (0, child_process_1.execSync)(`"${e}" config list -l --json`, {
      encoding: "utf-8",
      windowsHide: true
    });
    let o;
    try {
      o = JSON.parse(i);
      for (const [t, e] of Object.entries(o)) this.configuration.set(t, e);
    } catch (t) {
      this._log.error(t.message);
    }
    this._initialized = true;
  }
  getConfig(t) {
    return this.check(), this.configuration.get(t);
  }
  setConfig(t, e) {
    this.configuration.set(t, e);
  }
  check() {
    this._initialized || this.load();
  }
}
