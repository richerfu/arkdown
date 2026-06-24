/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/import-check-plugin.js
 */
import path_1 from "path";
import * as hvigor_arkts_base_1 from "@ohos/hvigor-arkts-base";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as constants_js_1 from "../util/constants";
import * as log_js_1 from "../util/log";
const LOGGER = hvigor_arkts_base_1.ArktsBaseLogger.getPluginLogger("ArkTS import check"),
  _log = hvigor_arkts_base_1.ArktsBaseLogger.getHvigorConsoleLogger("importCheckPlugin");
export function importCheckPlugin() {
  return {
    name: "importCheckPlugin",
    resolveId: resolveId,
    transform: transform,
    moduleParsed: moduleParsed
  };
}
function checkOutsideProjectModuleConfigMissing(t, o, e) {
  const r = t.share?.projectConfig,
    s = !o.startsWith(r?.projectTopDir) && !e.meta.belongProjectPath,
    i = r?.singleFileEmit;
  return s && i;
}
function resolveId(t, o, e, r) {
  const s = this.getModuleInfo(e);
  if (!s?.meta) return;
  if (t.includes("oh_modules")) {
    const o = `ERROR File: ${e}\n         Cannot import files outside of the current module using relative paths.\n         Import statement: '${t}'`;
    return void logMsg(this.share?.projectConfig?.strictMode?.noExternalImportByPath, o, "IMPORT_OUTSIDE_FILE_USING_RELATIVE_PATH", [t, e]);
  }
  if (path_1.isAbsolute(t)) {
    const o = s.meta.absoluteImports ?? [];
    return o.push(t), void (s.meta.absoluteImports = o);
  }
  const i = s?.meta?.belongModulePath;
  if (i && (t.startsWith("./") || t.startsWith("../"))) {
    if (!(0, hvigor_common_1.isChildPath)(o, i)) {
      const o = s.meta.outsideModuleImports ?? [];
      o.push(t), s.meta.outsideModuleImports = o;
    }
    checkOutsideProjectModuleConfigMissing(this, e, s) && (0, log_js_1.hvigorConsoleLoggerPrintErrorAndExit)(_log, "OUTSIDE_PROJECT_MODULE_MISSING_INFO", [e]);
  }
}
function transform(t, o) {
  if (o.includes(constants_js_1.OH_NODE_MODULES) || o.includes(constants_js_1.NODE_MODULES)) return;
  const e = this.getModuleInfo(o);
  e?.meta && (e.meta.belongModulePath = getBelongModulePath(o, this.share?.projectConfig?.allModulePaths) ?? e.meta.pkgPath);
}
function moduleParsed(t) {
  if (t?.meta?.absoluteImports && Array.isArray(t.meta.absoluteImports)) for (const o of t.meta.absoluteImports) {
    const e = `ERROR File: ${t.id}\n      Avoid absolute paths in imports. Import statement: '${o}'`;
    logMsg(this.share?.projectConfig?.strictMode?.noExternalImportByPath, e, "AVOID_ABSOLUTE_PATHS_IN_IMPORTS", [o, t.id]);
  }
  if (t?.meta?.outsideModuleImports && Array.isArray(t.meta.outsideModuleImports)) for (const o of t.meta.outsideModuleImports) {
    const e = `ERROR File: ${t.id}\n      Cannot import files outside of the current module using relative paths. Import statement: '${o}'`;
    logMsg(this.share?.projectConfig?.strictMode?.noExternalImportByPath, e, "IMPORT_OUTSIDE_FILE_USING_RELATIVE_PATH", [o, t.id]);
  }
}
function getBelongModulePath(t, o) {
  if (o) for (const e of o) if ((0, hvigor_common_1.isChildPath)(t, e)) return e;
}
function logMsg(t, o, e, r, s) {
  t ? (0, log_js_1.hvigorConsoleLoggerPrintError)(_log, e, r, s) : LOGGER.warn(o);
}
export default {
  resolveId: resolveId,
  transform: transform,
  moduleParsed: moduleParsed
};
