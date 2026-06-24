/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/native-binxo.js
 */
import path_1 from "path";
import fs_extra_1 from "fs-extra";
import * as base_command_builder_js_1 from "../../builder/base-command-builder";
import * as file_util_js_1 from "../../utils/file-util";
import * as libs_file_cache_util_js_1 from "../../utils/libs-file-cache-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as process_utils_js_1 from "../../utils/process-utils";
import * as so_strip_util_js_1 from "../../utils/so-strip-util";
const logger = ohos_logger_js_1.OhosLogger.getLogger("NativeBinxo");
export async function binxo(e) {
  await binxoNativeLibs(e);
}
async function binxoNativeLibs(e) {
  let i = [],
    t = [];
  i = so_strip_util_js_1.SoStripUtil.filesExcluding(e.collectAllLibs, e.excludeSoFromBinxo, e.intermediatesLibs), t = so_strip_util_js_1.SoStripUtil.matchedFiles(e.collectAllLibs, e.excludeSoFromBinxo, e.intermediatesLibs);
  const s = await readCache(e),
    a = i.map(async i => {
      const t = path_1.resolve(e.intermediatesLibs, i);
      if (!fs_extra_1.lstatSync(t).isDirectory()) return fs_extra_1.existsSync(t) && fs_extra_1.lstatSync(t).isSymbolicLink() ? copySoFile(i, e) : (await needBinxoCache(e, i, s)) ? executeBinxo(i, e) : undefined;
    }),
    l = t.map(async i => copySoFile(i, e));
  await Promise.all([...a, ...l]);
}
async function readCache(e) {
  const i = fs_extra_1.existsSync(e.lastCache) ? e.lastCache : e.cacheFilePath;
  return fs_extra_1.existsSync(i) ? JSON.parse(await fs_extra_1.readFile(i, {
    encoding: "utf-8"
  })) : {
    libs: {},
    binxo: {}
  };
}
async function needBinxoCache(e, i, t) {
  return !!(await libs_file_cache_util_js_1.LibsFileCacheUtil.isBinxoSourceChanged(e, path_1.resolve(e.intermediatesLibs, i), t, e.intermediatesLibs)) || libs_file_cache_util_js_1.LibsFileCacheUtil.isBinxoLibsChanged(path_1.resolve(e.binxoNativeLibs, i), t);
}
async function copySoFile(e, i) {
  const t = path_1.resolve(i.binxoNativeLibs, e),
    s = path_1.resolve(i.intermediatesLibs, e);
  fs_extra_1.lstatSync(s).isDirectory() || (await file_util_js_1.FileUtil.linkFile(s, t));
}
async function executeBinxo(e, i) {
  const t = path_1.resolve(i.binxoNativeLibs, e);
  fs_extra_1.ensureDirSync(path_1.dirname(t));
  const s = new BinxoCommandBuild(i.sdkBinxoPath).addAsan().addHwasanInputFile(path_1.resolve(i.intermediatesLibs, e)).addHwasanOutputFile(t).build();
  try {
    await new process_utils_js_1.ProcessUtils(i.moduleName, i.taskName).execute(s);
  } catch (t) {
    return t instanceof Error && t.message.includes("11801001") || logger.warn(t instanceof Error ? t.message : `${t}`), copySoFile(e, i);
  }
}
class BinxoCommandBuild extends base_command_builder_js_1.BaseCommandBuilder {
  constructor(e) {
    super([]), this.commandList.push(e);
  }
  addAsan() {
    return this.commandList.push("-A"), this;
  }
  addHwasanInputFile(e) {
    return this.commandList.push("--hwasan-inputfile"), this.commandList.push(`${e}`), this;
  }
  addHwasanOutputFile(e) {
    return this.commandList.push("--hwasan-outputfil"), this.commandList.push(`${e}`), this;
  }
}
