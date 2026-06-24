/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/utils.js
 */
import * as generateReportForHvigorw_js_1 from "@ohos/coverage/lib/src/generateReportForHvigorw";
import * as hvigor_arkts_base_1 from "@ohos/hvigor-arkts-base";
import fs_1 from "fs";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import os_1 from "os";
import path_1 from "path";
import * as constants_js_1 from "./constants";
const WINDOWS_OS_NAME = "Windows_NT",
  LINUX_OS_NAME = "Linux",
  logger = hvigor_arkts_base_1.ArktsBaseLogger.getLogger("ets-loader");
export async function generateReport(e, t, o) {
  try {
    await (0, generateReportForHvigorw_js_1.generateReportForHvigorw)(e, t, o);
  } catch (e) {
    logger.error(`GenerateReport ERROR: ${e}`);
  }
}
export function readFile(e, t) {
  try {
    fs_1.readdirSync(e).forEach(o => {
      const r = path_1.join(e, o);
      fs_1.statSync(r).isDirectory() ? readFile(r, t) : t.push(r);
    });
  } catch (e) {
    logger.error(`ETS ERROR: ${e}`);
  }
}
export function writeFileSync(e, t) {
  if (!fs_1.existsSync(e)) {
    const t = path_1.join(e, "..");
    fs_1.existsSync(t) && !fs_1.statSync(t).isFile() || mkDir(t);
  }
  fs_1.writeFileSync(e, t);
}
function mkDir(e) {
  const t = path_1.join(e, "..");
  fs_1.existsSync(t) && !fs_1.statSync(t).isFile() || mkDir(t), fs_1.mkdirSync(e);
}
export function toUnixPath(e) {
  return isWindows() ? path_1.posix.join(...e.split(path_1.sep)) : e;
}
function isWindows() {
  return "Windows_NT" === os_1.type();
}
export function isLinux() {
  return "Linux" === os_1.type();
}
export function isVirtualFile(e) {
  return e.startsWith(constants_js_1.VIRTUAL_FILE_PREFIX);
}
export function getJson5Obj(e, t = "utf-8") {
  if (!fs_1.existsSync(e)) return;
  const o = fs_1.readFileSync(e, {
    encoding: t
  });
  try {
    return (0, hvigor_common_1.parseJsonText)(o);
  } catch (e) {
    return;
  }
}
export function getPkgName2SourceRootsMap(e) {
  const t = {},
    o = e.pkgNameToPkgBriefInfo;
  return o ? (Object.values(o).forEach(e => {
    e.originalSourceRoots && (t[e.pkgName] = e.originalSourceRoots);
  }), t) : t;
}
export function getOhPackageJsonPath(e) {
  return composeOhPackagePathMap?.get(e) || e;
}
export function allowToPreloadSo(e) {
  return e?.isPreloadSoEnabled && "true" !== e?.widgetCompile;
}
export const composeOhPackagePathMap = new Map();
