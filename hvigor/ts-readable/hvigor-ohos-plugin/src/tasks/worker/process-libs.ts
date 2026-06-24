/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/process-libs.js
 */
import * as child_process_1 from "child_process";
import * as fs_1 from "fs";
import path_1 from "path";
import * as stream_1 from "stream";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as fs from "fs-extra";
import * as minimatch_1 from "minimatch";
import * as file_util_js_1 from "../../utils/file-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
const logger = ohos_logger_js_1.OhosLogger.getLogger("NativeStrip"),
  whiteListLibs = ["libc++_shared.so"],
  buildLibs = (e, t, i) => e.map(e => ({
    name: path_1.basename(e),
    path: path_1.resolve(i, e),
    relativePath: e,
    pkgName: t.getPackageName(),
    pkgVersion: t.getDependencyVersion(),
    isLocal: t.isLocal(),
    selected: 0
  })).filter(e => !fs.lstatSync(e.path).isDirectory());
export const filterBySelection = (e, t) => (0 === t.length || t.forEach(t => (0, applyForSelectionRule)(e, t)), e);
function filterLibs(e, t, i) {
  return t ? e.filter(e => t.includes(e.name)) : i ? e.filter(e => i.some(t => (0, minimatch_1.minimatch)(e.relativePath, t, {
    dot: true
  }))) : [];
}
export const applyForSelectionRule = (e, t) => {
  var i, r;
  const o = e.filter(e => e.pkgName === t.package).filter(e => !t.version || e.pkgVersion === t.version),
    a = filterLibs(o, t.include, t.includePattern);
  let s;
  a.forEach(e => e.selected = 1), a.length > 0 && (s = a.map(e => e.name).join(", "), logger.debug(`Selection rule :${t.package}:${null !== (i = t.version) && undefined !== i ? i : ""}:include hit for ${s}`));
  const n = filterLibs(o, t.exclude, t.excludePattern);
  n.forEach(e => e.selected = 2), n.length > 0 && (s = n.map(e => e.name).join(", "), logger.debug(`Selection rule:${t.package}:${null !== (r = t.version) && undefined !== r ? r : ""}:exclude hit for ${s}`));
};
export async function processLibs(e, t) {
  var i, r;
  const o = [...e.allHarLibs.reverse(), ...e.outputLibs, ...e.cangjieLibs, ...e.localLibs];
  (0, filterBySelection)(o, null !== (r = null === (i = e.filterRule) || undefined === i ? undefined : i.select) && undefined !== r ? r : []);
  const {
    excludes: a,
    pickFirsts: s,
    pickLasts: n,
    enableOverride: l
  } = {
    excludes: [],
    pickFirsts: [],
    pickLasts: [],
    enableOverride: false,
    ...e.filterRule
  };
  try {
    fs.rmSync(e.outputDir, {
      recursive: true,
      force: true,
      maxRetries: 3
    }), fs.mkdirSync(e.outputDir, {
      recursive: true
    });
  } catch (i) {
    if (i instanceof Error) {
      let r = e.outputDir;
      const o = i.path;
      "string" == typeof o && (r = o), t.printErrorExit("DELETE_FILE_ERROR", [r]);
    }
  }
  const c = new Map(),
    u = [];
  o.forEach(e => {
    if (2 !== e.selected && !u.includes(e.relativePath)) {
      if (1 === e.selected) return c.set(e.relativePath, [e.path]), void u.push(e.relativePath);
      if (a.some(t => (0, minimatch_1.minimatch)(e.relativePath, t))) logger.debug(`The filter condition excludes matched ${e.path}.`);else {
        if (!n.some(t => (0, minimatch_1.minimatch)(e.relativePath, t)) || !c.has(e.relativePath)) return s.some(t => (0, minimatch_1.minimatch)(e.relativePath, t)) || l ? (c.set(e.relativePath, [e.path]), void logger.debug(`The filter condition pickFirsts or enableOverride matched ${e.path}.`)) : void (c.has(e.relativePath) ? c.get(e.relativePath).push(e.path) : c.set(e.relativePath, [e.path]));
        logger.debug(`The filter condition pickLasts matched ${e.path}.`);
      }
    }
  }), await checkDuplicatedNativeLibs(c, e.profilePath, e.moduleName);
  for (const [t, i] of c) await file_util_js_1.FileUtil.linkFile(i.pop(), path_1.resolve(e.outputDir, t));
  e.isHarType || "true" !== hvigor_1.hvigorCore.getExtraConfig().get("ohos-enable-hwasan") || (await checkCompiledLibs(e), await checkPrecompileLibs(e));
}
async function checkPrecompileLibs(e) {
  const t = getNativeLibs(e),
    i = compareLibraryObjects(await getSubFolderLibs(e.outputDir), t),
    r = await getNoHwasanLibs(i, e.readElf);
  r.length && logHwasanWarning(r);
}
function getNativeLibs(e) {
  var t;
  const i = {};
  return null === (t = e.nativeLibs) || undefined === t || t.forEach(t => {
    t.forEach(t => {
      const r = t.getAbi(),
        o = t.getNameOnDisk();
      if (!e.localOutputDir || !r) return;
      const a = path_1.resolve(e.localOutputDir, r);
      a && o && (i[a] ? i[a].push(o) : i[a] = [o]);
    });
  }), i;
}
async function checkCompiledLibs(e) {
  var t;
  if (null === (t = e.compileCommandsPath) || undefined === t ? undefined : t.length) for (const t of e.compileCommandsPath) try {
    const e = await fs.promises.readFile(path_1.resolve(t), "utf8"),
      i = JSON.parse(e).filter(e => e.command && "string" == typeof e.command && !e.command.includes("fsanitize=hwaddress")).map(e => e.file);
    i.length && logHwasanWarning(i);
  } catch (e) {
    logger.warn(`Meet error when checking compilation script ${path_1.resolve(t)}, read debug log for more information.`), logger.debug(e);
  }
}
function logHwasanWarning(e) {
  logger.warn(`HWASAN is not enabled for the following products: \n  ${e.join("\n  ")}\n  Check the compilation script and ensure that the HWASAN of all native products is enabled.`);
}
async function getNoHwasanLibs(e, t) {
  const i = Object.entries(e);
  return (await Promise.all(i.flatMap(([e, i]) => i.map(async i => {
    try {
      const r = i.includes(".a"),
        o = r ? "-s" : "-d",
        a = r ? "__hwasan_init" : "libclang_rt.hwasan.so",
        {
          stdout: s
        } = await execPromise(`"${t}" ${o} "${e}${path_1.sep}${i}"`);
      return s.includes(a) ? null : `${e}${path_1.sep}${i}`;
    } catch (e) {
      return logger.warn(`Error executing HwasanLibs check for ${i}: ${e}`), null;
    }
  })))).filter(e => null !== e);
}
function execPromise(e) {
  return new Promise((t, i) => {
    (0, child_process_1.exec)(e, (e, r, o) => e ? (logger.debug(`exec error: ${e.message}`), void i(new Error(`exec error: ${e.message}`))) : o ? (logger.debug(`stderr: ${o}`), void i(new Error(`stderr: ${o}`))) : void t({
      stdout: r
    }));
  });
}
function compareLibraryObjects(e, t) {
  const i = {};
  for (const r in e) if (Object.prototype.hasOwnProperty.call(e, r)) {
    const o = e[r],
      a = Object.keys(t).find(e => getLastPathSegment(r) === getLastPathSegment(e));
    if (a) {
      const e = t[a],
        s = o.filter(t => !e.includes(t));
      s.length > 0 && (i[r] = s.filter(e => !whiteListLibs.includes(e)));
    } else i[r] = o.filter(e => !whiteListLibs.includes(e));
  }
  return i;
}
function getLastPathSegment(e) {
  const t = e.split(path_1.sep);
  return t[t.length - 1];
}
async function getSubFolderLibs(e) {
  if (!e) return {};
  try {
    const t = await fs.promises.readdir(e),
      i = {};
    return await Promise.all(t.map(async t => {
      const r = path_1.join(e, t);
      i[r] = await fs.promises.readdir(r);
    })), i;
  } catch (e) {
    return logger.debug("Error reading output directory:", e), {};
  }
}
async function checkDuplicatedNativeLibs(e, t, i) {
  const r = [];
  for (const t of e.keys()) {
    const i = e.get(t),
      o = path_1.basename(t);
    if (i && i.length > 1 && "libc++.so" !== o && "libc++_shared.so" !== o) {
      const e = await removeDuplicateFiles(i);
      if (e.length < 2) continue;
      r.push(...e);
    }
  }
  if (r.length) {
    const e = r.reduce((e, t) => `${e}- ${t}\n`, "");
    logger.printErrorExit("DUPLICATED_FILES_DOUND_IN_LIBPATH", [i, e.trimEnd()]);
  }
}
async function removeDuplicateFiles(e) {
  if (e.length < 2) return e;
  const t = [];
  for (const i of e) await collectUniqueFile(i, t);
  return t.map(e => e.file);
}
async function collectUniqueFile(e, t) {
  var i;
  const r = fs.lstatSync(e),
    o = r.isSymbolicLink() ? await fs.readlink(e) : e;
  if (0 !== t.length) for (const a of t) {
    const s = `${e} | ${a.file}`;
    if (!fs.existsSync(o)) {
      logger.debug(`Invalid file or symlink '${e}'.`);
      continue;
    }
    if (o === a.file) {
      logger.debug(`SymbolicLink relation between ${s}.`);
      continue;
    }
    if (r.size !== a.stats.size) {
      logger.debug(`Different file size for ${s}.`), t.push({
        file: o,
        stats: r
      });
      continue;
    }
    const n = await computeFileHash(o);
    a.hash = null !== (i = a.hash) && undefined !== i ? i : await computeFileHash(a.file), n === a.hash ? logger.debug(`Same checksum for ${s}.`) : (logger.debug(`Different checksum for ${s}.`), t.push({
      file: o,
      stats: r,
      hash: n
    }));
  } else t.push({
    file: o,
    stats: r
  });
}
async function computeFileHash(e) {
  const t = (0, hvigor_common_1.createHash)();
  return new Promise(i => (0, stream_1.pipeline)((0, fs_1.createReadStream)(e), t, () => i(t.digest("hex"))));
}
export { buildLibs };
