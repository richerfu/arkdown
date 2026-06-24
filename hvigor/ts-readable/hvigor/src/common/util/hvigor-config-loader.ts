/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/util/hvigor-config-loader.js
 */
import fs_1 from "fs";
import path_1 from "path";
import process_1 from "process";
import * as global_core_parameters_js_1 from "../../base/internal/data/global-core-parameters";
import * as const_js_1 from "../const/const";
import * as path_const_js_1 from "../const/path-const";
import * as parse_json_file_js_1 from "./parse-json-file";
export class HvigorConfigLoader {
  constructor() {
    this.configPropertyMap = new Map();
    const e = path_1.resolve(path_const_js_1.HVIGOR_PROJECT_WRAPPER_HOME, const_js_1.DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME);
    if (!fs_1.existsSync(e)) return;
    const t = parse_json_file_js_1.ParseJsonFile.parseJsonFile(e),
      o = path_1.resolve(path_const_js_1.HVIGOR_USER_HOME, const_js_1.DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME);
    let i;
    fs_1.existsSync(o) && (i = parse_json_file_js_1.ParseJsonFile.parseJsonFile(o), t.properties = {
      ...i.properties,
      ...t.properties
    }), this.hvigorConfig = t;
  }
  static init(e) {
    var t, o;
    if (undefined === e) return void (process_1.env.config = undefined);
    const i = HvigorConfigLoader.getConfigs();
    let r = {};
    null === (t = e.config) || undefined === t || t.forEach(e => {
      const t = e.split("=");
      2 === t.length && (r[t[0]] = t[t.length - 1], this.initCommandLineProperties(t[0], t[t.length - 1]));
    }), Array.isArray(e.prop) && (null === (o = e.prop) || undefined === o || o.forEach(e => {
      const t = e.split("=");
      2 === t.length && (r[t[0]] = t[t.length - 1], this.initCommandLineProperties(t[0], t[t.length - 1]));
    })), r = {
      ...i,
      ...r
    }, process_1.env.config = JSON.stringify(r);
  }
  static initCommandLineProperties(e, t) {
    if (!e.startsWith(`${const_js_1.PROPERTIES + const_js_1.DOT}`)) return;
    const o = e.substring(`${const_js_1.PROPERTIES + const_js_1.DOT}`.length);
    global_core_parameters_js_1.coreParameter.properties[o] = this.convertToParamValue(t);
  }
  static convertToParamValue(e) {
    let t = Number(e);
    return e.length <= 16 && !isNaN(t) ? t : (t = "true" === e || "false" !== e && t, "boolean" == typeof t ? t : e.trim());
  }
  getHvigorConfig() {
    return this.hvigorConfig;
  }
  getPropertiesConfigValue(e) {
    var t;
    if (this.configPropertyMap.has(e)) return this.configPropertyMap.get(e);
    const o = HvigorConfigLoader.getConfigs()["properties.".concat(e)],
      i = undefined !== process_1.env.config && null !== (t = JSON.parse(process_1.env.config)["properties.".concat(e)]) && undefined !== t ? t : o;
    if (undefined !== i) {
      const t = this.parseConfigValue(i);
      return this.configPropertyMap.set(e, t), t;
    }
    if (undefined === this.hvigorConfig) return void this.configPropertyMap.set(e, undefined);
    const r = this.hvigorConfig.properties ? this.hvigorConfig.properties[e] : undefined;
    return this.configPropertyMap.set(e, r), r;
  }
  static clean() {
    HvigorConfigLoader.instance && (HvigorConfigLoader.instance = undefined), HvigorConfigLoader.config && (HvigorConfigLoader.config = undefined);
  }
  static getInstance() {
    return HvigorConfigLoader.instance || (HvigorConfigLoader.instance = new HvigorConfigLoader()), HvigorConfigLoader.instance;
  }
  static getConfigs() {
    if (this.config) return this.config;
    const e = process_1.argv.slice(2),
      t = /^(--config|-c).*/,
      o = /^(--config|-c)$/,
      i = {};
    for (const [r, s] of e.entries()) if (o.test(s)) {
      if (r + 1 < e.length) {
        const t = e[r + 1].split("=");
        this.setConfig(t, i);
      }
    } else if (t.test(s)) {
      const e = s.match(t);
      if (e && e[0].length < s.length) {
        const t = s.substring(e[0].length).split("=");
        this.setConfig(t, i);
      }
    }
    return this.config = i, i;
  }
  static setConfig(e, t) {
    2 === e.length && (t[e[0]] = e[e.length - 1]);
  }
  parseConfigValue(e) {
    if ("true" === e.toLowerCase()) return true;
    if ("false" === e.toLowerCase()) return false;
    const t = Number(e);
    return isNaN(t) ? e : t;
  }
}
HvigorConfigLoader.config = undefined;
