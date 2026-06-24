/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/npm-utils.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_2 from "@ohos/hvigor";
import * as fse from "fs-extra";
import path_1 from "path";
import * as tar_1 from "tar";
import * as file_util_js_1 from "./file-util";
import * as hvigor_common_1 from "@ohos/hvigor-common";
const isFileSpec = (0, hvigor_2.isWindows)() ? /^(?:[.]|~[/]|[/\\]|[a-zA-Z]:)/ : /^(?:[.]|~[/]|[/]|[a-zA-Z]:)/,
  hasSlashes = (0, hvigor_2.isWindows)() ? /\\|[/]/ : /[/]/,
  isFileName = /[.](?:tgz|tar.gz|tar)$/i;
export function checkHasLocalFileDependency(e, t, i) {
  const o = t;
  return undefined !== o.dependencies && Object.values(o.dependencies).some(t => {
    if (false !== (null == i ? undefined : i.allowInside) && isLocalDependency(t) && t) {
      const i = /^file:(.*)/i.test(t) ? /^file:(.*)/i.exec(t)[1] : t;
      return !file_util_js_1.FileUtil.isSubDir(path_1.dirname(e), path_1.resolve(path_1.dirname(e), i));
    }
    return isLocalDependency(t);
  });
}
export function collectLocalFileDependency(e, t) {
  const i = [],
    o = t;
  return undefined === o.dependencies || Object.values(o.dependencies).forEach(t => {
    if (isLocalDependency(t) && t) {
      const o = /^file:(.*)/i.test(t) ? /^file:(.*)/i.exec(t)[1] : t;
      file_util_js_1.FileUtil.isSubDir(path_1.dirname(e), path_1.resolve(path_1.dirname(e), o)) && i.push(o);
    }
  }), i;
}
export function isLocalDependency(e) {
  return !!e && (isFileSpec.test(e) || /^file:/i.test(e) || hasSlashes.test(e) || isFileName.test(e));
}
export function getNpmPath() {
  return process.execPath;
}
export async function execOhpmPack(e, t, i, o = true) {
  const c = "execute ohpm packaging command",
    r = i.createSubEvent(c, "");
  r.start(), await (0, tar_1.create)({
    cwd: e,
    noDirRecurse: false,
    file: t,
    gzip: true,
    prefix: o ? "package" : undefined
  }, await fse.readdir(e)), r.stop(), r.setLog(c, hvigor_1.MetricLogType.INFO);
}
const REGEX_ETS = /(.+)\.ets$/,
  REGEX_TS = /(.+)\.ts$/,
  REGEX_D = /(.+)\.d$/,
  getTypesFieldFromMainField = e => undefined === e || "" === e ? "index.d.ets" : REGEX_ETS.test(e) ? e.replace(REGEX_ETS, "$1.d.ets") : REGEX_TS.test(e) ? e.replace(REGEX_TS, "$1.d.ts") : REGEX_D.test(e) ? e : `${e}.d`;
export function copyParameterizationLocalProductDependencies(e) {
  const {
    localProductDependencies: t,
    moduleDir: i,
    tempDir: o,
    logger: c
  } = e;
  i && o && (null == c || c.debug("These local product dependencies should be copied to har.", {
    moduleDir: i,
    localProductDependencies: t
  }), null == t || t.forEach(e => {
    if ((0, hvigor_common_1.isSubPath)(e, i)) {
      const t = path_1.relative(i, e),
        r = path_1.resolve(o, t);
      if (fse.existsSync(r)) return;
      fse.ensureDirSync(path_1.dirname(r)), fse.copyFileSync(e, r), null == c || c.debug("This local product dependency inside the module has been copied to har.", {
        depPath: e,
        moduleDir: i,
        copyTo: r
      });
    } else {
      const t = path_1.basename(e),
        i = path_1.resolve(o, t);
      if (fse.existsSync(i)) return;
      fse.copyFileSync(e, i), null == c || c.debug("This local product dependency outside the module has been copied to har.", {
        depPath: e,
        copyTo: i
      });
    }
  }));
}
export { getTypesFieldFromMainField };
