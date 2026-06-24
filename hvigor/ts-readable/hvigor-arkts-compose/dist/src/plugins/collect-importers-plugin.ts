/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/collect-importers-plugin.js
 */
import path_1 from "path";
import * as hvigor_arkts_base_1 from "@ohos/hvigor-arkts-base";
import fs_extra_1 from "fs-extra";
import * as utils_js_1 from "../util/utils";
const LOGGER = hvigor_arkts_base_1.ArktsBaseLogger.getPluginLogger("changedFileList collect importers");
export function collectImportersPlugin(e) {
  let t = [];
  return {
    name: "collectImportersPlugin",
    options: function (e) {
      t = Object.values(e.input);
    },
    buildEnd: function () {
      if (!e.collectImportersConfig) return;
      const {
          relativeDir: i,
          buildConfigJsonPath: s,
          isReload: o,
          patchPath: r
        } = e.collectImportersConfig,
        l = e.aceBuildJson;
      if (!(i && s && fs_extra_1.existsSync(s) && l && fs_extra_1.existsSync(l))) return;
      const n = (0, utils_js_1.getJson5Obj)(l);
      if (!n.patchConfig || !n.patchConfig.oldMapFilePath || undefined === n.patchConfig.mode) return;
      const a = path_1.resolve(r, "ets.json");
      if (fs_extra_1.ensureDirSync(r), !o) {
        const e = getCompiledFiles(this, t);
        return void fs_extra_1.writeJSONSync(a, {
          compiledFileList: e
        });
      }
      const c = (0, utils_js_1.getJson5Obj)(a) ?? {},
        d = c?.compiledFileList ?? [],
        u = n?.patchConfig?.changedFileList;
      let f = [];
      u && fs_extra_1.existsSync(u) ? f = checkModifiedFiles({
        changedFileListPath: u,
        relativeDir: i,
        compiledFileList: d
      }) : LOGGER.debug(`changedFileList not exists: ${u}`), c.isImportNewFile = isImportNewFile(this, f.map(e => path_1.resolve(i, e)), d), fs_extra_1.writeJSONSync(a, c);
    }
  };
}
function checkModifiedFiles({
  changedFileListPath: e,
  relativeDir: t,
  compiledFileList: i
}) {
  const s = (0, utils_js_1.getJson5Obj)(e),
    o = s?.modifiedFiles ?? [],
    r = [];
  let l = false;
  LOGGER.debug(`Start to collect modifiedFiles importers in ${e}`), LOGGER.debug(`origin changedFileList modifiedFiles: ${o}`);
  for (let e = 0; e < o.length; e++) {
    const s = o[e],
      n = path_1.resolve(t, s);
    i.includes(n) ? r.push(o[e]) : l = true;
  }
  return l ? (s.modifiedFiles = r, fs_extra_1.writeJSONSync(e, s), LOGGER.debug(`changedFileList modifiedFiles change to: ${r}`)) : LOGGER.debug("changedFileList not changed"), s.modifiedFiles;
}
function getCompiledFiles(e, t) {
  const i = new Set(),
    s = [...t];
  for (; s.length > 0;) {
    const t = s.shift();
    t && (i.add(t), Array.from(e.getModuleInfo(t)?.importedIds ?? []).forEach(e => !i.has(e) && s.push(e)));
  }
  return [...i];
}
function isImportNewFile(e, t, i) {
  const s = [];
  return t.forEach(t => {
    s.push(...Array.from(e.getModuleInfo(t)?.importedIds ?? []));
  }), s.some(e => !i.includes(e));
}
