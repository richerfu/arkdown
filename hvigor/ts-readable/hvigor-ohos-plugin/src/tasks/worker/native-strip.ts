/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/native-strip.js
 */
import path_1 from "path";
import fs_extra_1 from "fs-extra";
import * as glob_1 from "glob";
import * as base_command_builder_js_1 from "../../builder/base-command-builder";
import * as common_const_js_1 from "../../const/common-const";
import * as file_util_js_1 from "../../utils/file-util";
import * as libs_file_cache_util_js_1 from "../../utils/libs-file-cache-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as process_utils_js_1 from "../../utils/process-utils";
import * as so_strip_util_js_1 from "../../utils/so-strip-util";
const logger = ohos_logger_js_1.OhosLogger.getLogger("NativeStrip");
export async function strip(e) {
  so_strip_util_js_1.SoStripUtil.needStrip(e.debugSymbol) ? await stripNativeLibs(e) : await copyFiles(e);
}
export function clearExpiredSoInStrippedDir(e, t) {
  cleanStrippedSo(e, t), cleanSymbolicLink(e);
}
async function stripNativeLibs(e) {
  let t = [],
    i = [];
  e.strip ? (t = so_strip_util_js_1.SoStripUtil.filesExcluding(e.collectAllLibs, e.exclude, e.intermediatesProcessLibs), i = so_strip_util_js_1.SoStripUtil.matchedFiles(e.collectAllLibs, e.exclude, e.intermediatesProcessLibs)) : e.strip || (t = so_strip_util_js_1.SoStripUtil.matchedFiles(e.collectAllLibs, e.exclude, e.intermediatesProcessLibs), i = so_strip_util_js_1.SoStripUtil.filesExcluding(e.collectAllLibs, e.exclude, e.intermediatesProcessLibs));
  const s = await readCache(e),
    r = t.map(async t => {
      const i = path_1.resolve(e.intermediatesProcessLibs, t);
      if (!fs_extra_1.lstatSync(i).isDirectory()) return fs_extra_1.existsSync(i) && fs_extra_1.lstatSync(i).isSymbolicLink() ? copySoFile(t, e) : (await needStripCache(e, t, s)) ? executeStrip(t, e) : undefined;
    }),
    a = i.map(async t => copySoFile(t, e));
  await Promise.all([...r, ...a]);
}
async function copyFiles(e) {
  logger.debug("Do not need strip, copying .so files to stripped_native_libs");
  const t = glob_1.glob.sync(true === e.collectAllLibs ? common_const_js_1.NativeConst.LIBRARY_ALL_FILE_PATTERN : common_const_js_1.NativeConst.LIBRARY_FILE_PATTERN, {
    cwd: e.intermediatesProcessLibs
  }).map(async t => {
    const i = path_1.resolve(e.strippedNativeLibs, t),
      s = path_1.resolve(e.intermediatesProcessLibs, t);
    fs_extra_1.lstatSync(s).isDirectory() || (await file_util_js_1.FileUtil.linkFile(s, i));
  });
  await Promise.all(t);
}
async function readCache(e) {
  const t = fs_extra_1.existsSync(e.lastCache) ? e.lastCache : e.cacheFilePath;
  return fs_extra_1.existsSync(t) ? JSON.parse(await fs_extra_1.readFile(t, {
    encoding: "utf-8"
  })) : {
    libs: {},
    stripped: {}
  };
}
async function needStripCache(e, t, i) {
  return !!(await libs_file_cache_util_js_1.LibsFileCacheUtil.isSourceChanged(e, path_1.resolve(e.intermediatesProcessLibs, t), i, e.intermediatesProcessLibs, e.debugSymbol)) || libs_file_cache_util_js_1.LibsFileCacheUtil.isSinkChanged(path_1.resolve(e.strippedNativeLibs, t), i);
}
function cleanStrippedSo(e, t) {
  if (!fs_extra_1.existsSync(e)) return;
  function i(e) {
    return fs_extra_1.readdirSync(e).reduce((t, s) => {
      const r = path_1.resolve(e, s);
      if (!fs_extra_1.existsSync(r)) return t;
      return fs_extra_1.lstatSync(r).isDirectory() ? t.concat(i(r)) : t.concat(r);
    }, []);
  }
  const s = i(e),
    r = new Set(i(t).map(e => path_1.relative(t, e)));
  s.filter(t => {
    const i = path_1.relative(e, t);
    return !r.has(i);
  }).forEach(e => {
    try {
      fs_extra_1.unlinkSync(e);
    } catch (t) {
      logger.error(`delete filePath: ${e} has error: ${t}`);
    }
  });
}
function cleanSymbolicLink(e) {
  if (!fs_extra_1.existsSync(e)) return;
  const t = fs_extra_1.lstatSync(e);
  t.isSymbolicLink() ? fs_extra_1.removeSync(e) : t.isDirectory() && fs_extra_1.readdirSync(e).forEach(t => {
    cleanSymbolicLink(path_1.resolve(e, t));
  });
}
async function copySoFile(e, t) {
  const i = path_1.resolve(t.strippedNativeLibs, e),
    s = path_1.resolve(t.intermediatesProcessLibs, e);
  fs_extra_1.lstatSync(s).isDirectory() || (await file_util_js_1.FileUtil.linkFile(s, i));
}
async function executeStrip(e, t) {
  const i = path_1.resolve(t.strippedNativeLibs, e);
  fs_extra_1.ensureDirSync(path_1.dirname(i));
  const s = new LlvmCommandBuild(t.sdkLlvmStripPath).addKeepSection(".permission").addSourceFilePath(path_1.resolve(t.intermediatesProcessLibs, e)).addOutputFilePath(i).build();
  try {
    await new process_utils_js_1.ProcessUtils(t.moduleName, t.taskName).execute(s);
  } catch (i) {
    return logger.warn(i), copySoFile(e, t);
  }
}
class LlvmCommandBuild extends base_command_builder_js_1.BaseCommandBuilder {
  constructor(e) {
    super([]), this.commandList.push(e);
  }
  addKeepSection(e) {
    return this.commandList.push(`--keep-section=${e}`), this;
  }
  addOutputFilePath(e) {
    return this.commandList.push(`-o${e}`), this;
  }
  addSourceFilePath(e) {
    return this.commandList.push(`${e}`), this;
  }
}
