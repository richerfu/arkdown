/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/oh-exports-util.js
 */
import path_1 from "path";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as copy_resources_util_js_1 from "./copy-resources-util";
const JS_OR_TS_FILES_SUFFIX = [".js", ".ts"];
export function rewriteOhExports(t, e, s) {
  const r = [];
  return s.forEach(s => {
    const o = path_1.resolve(t, s);
    if (!fs_extra_1.existsSync(o)) return;
    if (fs_extra_1.statSync(o).isDirectory()) return void r.push((0, copy_resources_util_js_1.toUnixPath)(path_1.relative(t, o)));
    const _ = path_1.resolve(e, s);
    if (fs_extra_1.existsSync(_) && r.push((0, copy_resources_util_js_1.toUnixPath)(path_1.relative(e, _))), !s.endsWith(".d.ets") && !s.endsWith(".d.ts") && (s.endsWith(".ets") || s.endsWith(".ts"))) {
      const t = path_1.extname(s),
        o = path_1.resolve(e, s.replace(new RegExp(`${t}$`), `.d${t}`));
      fs_extra_1.existsSync(o) && r.push((0, copy_resources_util_js_1.toUnixPath)(path_1.relative(e, o))), JS_OR_TS_FILES_SUFFIX.forEach(o => {
        const _ = path_1.resolve(e, s.replace(new RegExp(`${t}$`), o));
        fs_extra_1.existsSync(_) && r.push((0, copy_resources_util_js_1.toUnixPath)(path_1.relative(e, _)));
      });
    }
  }), r;
}
export function collectOhExportsFilePaths(t, e) {
  const s = [];
  return e.forEach(e => {
    const r = path_1.resolve(t, e);
    if (!fs_extra_1.existsSync(r)) return;
    const o = fs_extra_1.statSync(r);
    o.isDirectory() ? s.push(...(0, hvigor_common_1.traverseFilesSync)(r, /\.(ets|ts|js)$/)) : o.isFile() && s.push(r);
  }), s;
}
