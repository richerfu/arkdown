/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/hvigor-config-reader.js
 */
import fs_1 from "fs";
import path_1 from "path";
import process_1 from "process";
import * as const_js_1 from "../../common/const/const";
import * as path_const_js_1 from "../../common/const/path-const";
import * as hvigor_config_loader_js_1 from "../../common/util/hvigor-config-loader";
import * as json5_reader_js_1 from "./json5-reader";
export const defaultOptions = {
  maxOldSpaceSize: 8192,
  maxSemiSpaceSize: 16,
  exposeGC: true
};
export class HvigorConfigReader extends json5_reader_js_1.Json5Reader {
  static getHvigorConfig() {
    const e = path_1.resolve(path_const_js_1.HVIGOR_PROJECT_WRAPPER_HOME, const_js_1.DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME);
    if (!fs_1.existsSync(e)) return;
    const t = this.getJson5Obj(e),
      i = path_1.resolve(path_const_js_1.HVIGOR_USER_HOME, const_js_1.DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME);
    let r;
    return fs_1.existsSync(i) && (r = this.getJson5Obj(i), t.properties = {
      ...r.properties,
      ...t.properties
    }, t.parameterFile || (t.parameterFile = r.parameterFile)), t;
  }
  static getBuildParamFilePath() {
    const e = path_1.resolve(path_const_js_1.HVIGOR_PROJECT_WRAPPER_HOME, const_js_1.DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME);
    if (!fs_1.existsSync(e)) return;
    const t = this.getJson5Obj(e);
    if (t.parameterFile) return HvigorConfigReader.normalizeParameterFilePath(t.parameterFile, path_const_js_1.HVIGOR_PROJECT_WRAPPER_HOME);
    const i = path_1.resolve(path_const_js_1.HVIGOR_USER_HOME, const_js_1.DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME);
    if (!fs_1.existsSync(i)) return;
    return this.getJson5Obj(i).parameterFile ? HvigorConfigReader.normalizeParameterFilePath(t.parameterFile, path_const_js_1.HVIGOR_USER_HOME) : undefined;
  }
  static normalizeParameterFilePath(e, t) {
    if (!path_1.isAbsolute(e)) return path_1.resolve(t, e);
  }
  static getPropertiesConfigValue(e) {
    return hvigor_config_loader_js_1.HvigorConfigLoader.getInstance().getPropertiesConfigValue(e);
  }
  static getMaxOldSpaceSize(e = false) {
    var t, i, r, a;
    const o = process_1.argv.find(e => e.startsWith(this.maxOldSpaceSizeParamPrefiex)),
      s = Number(null !== (t = null == o ? undefined : o.slice((null == o ? undefined : o.indexOf("=")) + 1)) && undefined !== t ? t : ""),
      n = null === (r = null === (i = HvigorConfigReader.getHvigorConfig()) || undefined === i ? undefined : i.nodeOptions) || undefined === r ? undefined : r.maxOldSpaceSize,
      _ = process_1.execArgv.find(e => e.startsWith(this.maxOldSpaceSizeParamPrefiex)),
      l = {
        argv: s,
        config: n,
        execArgv: Number(null !== (a = null == _ ? undefined : _.slice((null == _ ? undefined : _.indexOf("=")) + 1)) && undefined !== a ? a : ""),
        default: defaultOptions.maxOldSpaceSize
      };
    return this.getPriorVal(l, e);
  }
  static getStacktrace(e = false) {
    var t, i, r;
    const a = [...process_1.argv].reverse().find(e => e === this.STACK_TRACE || e === this.NO_STACK_TRACE);
    let o;
    o = a === this.STACK_TRACE || a !== this.NO_STACK_TRACE && undefined;
    const s = null === (i = null === (t = HvigorConfigReader.getHvigorConfig()) || undefined === t ? undefined : t.debugging) || undefined === i ? undefined : i.stacktrace,
      n = false;
    return e ? null != s ? s : n : null !== (r = null != o ? o : s) && undefined !== r ? r : n;
  }
  static getMaxSemiSpaceSize(e = false) {
    var t, i, r, a;
    const o = process_1.argv.find(e => e.startsWith(this.maxSemiSpaceSizeParamPrefiex)),
      s = Number(null !== (t = null == o ? undefined : o.slice((null == o ? undefined : o.indexOf("=")) + 1)) && undefined !== t ? t : ""),
      n = null === (r = null === (i = HvigorConfigReader.getHvigorConfig()) || undefined === i ? undefined : i.nodeOptions) || undefined === r ? undefined : r.maxSemiSpaceSize,
      _ = process_1.execArgv.find(e => e.startsWith(this.maxSemiSpaceSizeParamPrefiex)),
      l = {
        argv: s,
        config: n,
        execArgv: Number(null !== (a = null == _ ? undefined : _.slice((null == _ ? undefined : _.indexOf("=")) + 1)) && undefined !== a ? a : ""),
        default: defaultOptions.maxSemiSpaceSize
      };
    return this.getPriorVal(l, e);
  }
  static getNodeParamFromProcessArgv() {
    const e = [];
    for (const t of [this.maxOldSpaceSizeParamPrefiex, this.maxSemiSpaceSizeParamPrefiex]) {
      const i = process_1.argv.find(e => e.startsWith(t));
      i && e.push(i);
    }
    return e;
  }
  static getPriorVal(e, t) {
    return t ? e.config || e.default : e.argv || e.config || e.execArgv || e.default;
  }
}
HvigorConfigReader.maxOldSpaceSizeParamPrefiex = "--max-old-space-size=", HvigorConfigReader.maxSemiSpaceSizeParamPrefiex = "--max-semi-space-size=", HvigorConfigReader.STACK_TRACE = "--stacktrace", HvigorConfigReader.NO_STACK_TRACE = "--no-stacktrace";
