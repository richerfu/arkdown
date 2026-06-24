/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/os-country-util.js
 */
import * as child_process_1 from "child_process";
import * as system_util_js_1 from "./system-util";
const WIN_CODE_CN = 2052;
export var countryEnum;
let countryCache;
export function getOsLanguage() {
  if (countryCache) return countryCache;
  let t = countryEnum.CN;
  return (0, system_util_js_1.isWindows)() ? t = getWinLocale() : (0, system_util_js_1.isMac)() ? t = getMacLocale() : (0, system_util_js_1.isLinux)() && (t = getLinuxLocale()), process.env["user.country"] && (t = "CN" === process.env["user.country"].toString() ? countryEnum.CN : countryEnum.EN), countryCache = t, t;
}
function getWinLocale() {
  const t = (0, child_process_1.spawnSync)("wmic", ["os", "get", "locale"]);
  if (0 !== t.status) return countryEnum.CN;
  return 2052 === Number.parseInt(t.stdout.toString().replace("Locale", ""), 16) ? countryEnum.CN : countryEnum.EN;
}
function getMacLocale() {
  const t = (0, child_process_1.spawnSync)("defaults", ["read", "-globalDomain", "AppleLocale"]);
  return 0 !== t.status || t.stdout.toString().indexOf("zh_CN") >= 0 ? countryEnum.CN : countryEnum.EN;
}
function getLinuxLocale() {
  var t;
  const n = (0, child_process_1.spawnSync)("locale");
  if (0 !== n.status) return countryEnum.CN;
  const e = {};
  for (const u of n.stdout.toString().split("\n")) {
    const [n, o] = u.split("=");
    e[n] = null !== (t = null == o ? undefined : o.replace(/^"|"$/g, "")) && undefined !== t ? t : "";
  }
  const u = e.LC_ALL || e.LC_MESSAGES || e.LANG || e.LANGUAGE;
  return u && u.indexOf("zh_CN") >= 0 ? countryEnum.CN : countryEnum.EN;
}
!function (t) {
  t.CN = "cn", t.EN = "en";
}(countryEnum = countryEnum || (countryEnum = {}));
