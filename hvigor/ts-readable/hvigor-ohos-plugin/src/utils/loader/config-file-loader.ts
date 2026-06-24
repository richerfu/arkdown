/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/loader/config-file-loader.js
 */
class ConfigFileLoader {
  constructor() {
    this.configFileJsonMap = new Map();
  }
  clean() {
    this.configFileJsonMap.clear();
  }
  getConfigFileJson(e) {
    if (this.configFileJsonMap.has(e)) return this.configFileJsonMap.get(e);
  }
  setConfigFileJson(e, i) {
    this.configFileJsonMap.set(e, i);
  }
}
export const configFileLoader = new ConfigFileLoader();
