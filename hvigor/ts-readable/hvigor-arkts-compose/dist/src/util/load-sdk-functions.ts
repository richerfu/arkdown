/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/load-sdk-functions.js
 */
import * as buildInstrument_1 from "@ohos/coverage/lib/src/plugin/buildInstrument";
import * as hvigor_arkts_base_1 from "@ohos/hvigor-arkts-base";
import * as plugin_babel_1 from "@rollup/plugin-babel";
import plugin_terser_1 from "@rollup/plugin-terser";
import fs_1 from "fs";
import path_1 from "path";
import * as arkts_pack_js_1 from "../arkts-pack";
import * as constants_js_1 from "./constants";
const GEN_ABC_PLUGIN_NAME = "genAbc",
  logger = hvigor_arkts_base_1.ArktsBaseLogger.getLogger("load_sdk");
export const cleanConfig = [];
export const sdkPlugins = [];
export const sdkSupportExtensions = [];
let sdkCache = [];
const hijackTscCompilerOptionsPaths = (e, t) => {
  const s = path_1.join(e, "lib", "ets_checker.js");
  if (!fs_1.existsSync(s)) throw new Error("load etsChecker Error.Cause: file not exists");
  const r = require(s).compilerOptions;
  if (!r) return;
  const o = Object.getOwnPropertyDescriptor(r, "paths");
  if (!o || o.configurable) {
    let e = t;
    Object.defineProperty(r, "paths", {
      enumerable: true,
      configurable: true,
      get: () => e,
      set(t) {
        e && Object.prototype.hasOwnProperty.call(e, "*") && t && Object.prototype.hasOwnProperty.call(t, "*") && (e["*"] = Array.from(new Set([...e["*"], ...t["*"]])), delete t["*"]), e = {
          ...e,
          ...t
        };
      }
    });
  }
};
export function loadSdkFunctions(e) {
  const t = loadScript(e.etsLoaderPath, e.apiVersion, e.etsLoaderVersion, e.isFaMode);
  hijackTscCompilerOptionsPaths(e.etsLoaderPath, e.otherPaths);
  const s = t.initConfig();
  entryObj = s.entryObj, e.globalModulePaths = s.globalModulePaths ?? [], e.obfuscationOptions && (e.obfuscationOptions.sdkApis = e.globalModulePaths);
  let r = e.obfuscationOptions?.selfConfig?.ruleOptions?.enable;
  const o = e.compileSdkVersion;
  o && o > 9 && r && undefined === t.generateConsumerObConfigFile && (logger.warn("The SDK does not support obfuscation. Please upgrade the SDK."), r = undefined);
  const n = undefined === r && e.buildMode?.toLowerCase() === constants_js_1.RELEASE,
    a = s.cardEntryObj;
  if (widgetEntryObj = undefined === a ? entryObj : a, cleanConfig.push(...t.getCleanConfig(s.workerFile)), sdkSupportExtensions.push(...(t.resolveFileExtensions ?? [])), t.sdkPlugins) {
    let s = t.sdkPlugins(e);
    if (t.sdkPluginsMap && t.sdkPluginsMap.genAbc) {
      const e = t.sdkPluginsMap.genAbc;
      s.splice(e, 0, (0, buildInstrument_1.buildInstrument)());
    } else s.push((0, buildInstrument_1.buildInstrument)());
    s = s.filter(e => "visualTransform" !== e.name), sdkPlugins.push(...s);
  } else sdkPlugins.push(t.watchChangeFiles(), t.etsChecker(), t.etsTransform(), t.apiTransform(), (0, buildInstrument_1.buildInstrument)(), t.genAbc(), n && terserPlugin(), e.compileMode === constants_js_1.JSBUNDLE && babelPlugin(e));
}
function removeSdkCache() {
  sdkCache.forEach(e => delete require.cache[e]);
}
function updateSdkCache(e) {
  sdkCache = [], Object.keys(require.cache).forEach(t => {
    e.has(t) || sdkCache.push(t);
  });
}
export function clearSdkContext() {
  entryObj = {}, widgetEntryObj = {}, sdkPlugins = [], cleanConfig = [], sdkSupportExtensions = [];
}
export async function runEtsStandaloneChecker(e) {
  hvigor_arkts_base_1.ArktsBaseLogger.clearLogEvents();
  const t = loadScript(e.config.etsLoaderPath, e.config.apiVersion, e.config.etsLoaderVersion, e.config.isFaMode);
  if (t.etsStandaloneChecker && "function" == typeof t.etsStandaloneChecker) {
    e.config.resolveModulePaths = (0, arkts_pack_js_1.getResolveModules)(e.config);
    const s = hvigor_arkts_base_1.ArktsBaseLogger.getLogger("lint");
    return t.etsStandaloneChecker(e.entry, s, e.config), hvigor_arkts_base_1.ArktsBaseLogger.getLogEvents();
  }
  return [];
}
function loadScript(e, t, s, r) {
  const o = path_1.join(e, "compile_plugin.js");
  if (!fs_1.existsSync(o)) throw new Error("load Sdk Functions Error.Cause: plugin export entry file not exists");
  const n = new Set(),
    a = r || t < constants_js_1.API_VERSION_11;
  a && (removeSdkCache(), Object.keys(require.cache).forEach(e => n.add(e)));
  const i = require(o);
  return a && updateSdkCache(n), i;
}
export async function generateObfuscationFile(e, t, s, r, o, n) {
  const a = loadScript(t, s, r, o);
  a.generateConsumerObConfigFile && "function" == typeof a.generateConsumerObConfigFile && a.generateConsumerObConfigFile(e, n);
}
const babelPlugin = e => (0, plugin_babel_1.getBabelInputPlugin)({
    extensions: [".js"],
    exclude: [e.packageManagerType === constants_js_1.OHPM ? constants_js_1.OH_NODE_MODULES : constants_js_1.NODE_MODULES],
    skipPreflightCheck: true,
    babelHelpers: "runtime",
    plugins: [require("@babel/plugin-proposal-class-properties")],
    compact: false
  }),
  terserPlugin = () => (0, plugin_terser_1)({
    compress: {
      defaults: false,
      dead_code: true,
      collapse_vars: true,
      unused: true,
      drop_debugger: true,
      if_return: true,
      reduce_vars: true,
      join_vars: false,
      sequences: 0
    },
    format: {
      semicolons: false,
      beautify: true,
      braces: true,
      indent_level: 2
    }
  });
export { entryObj, widgetEntryObj };
