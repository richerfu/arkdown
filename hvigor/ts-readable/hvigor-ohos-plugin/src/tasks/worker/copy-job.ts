/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/copy-job.js
 */
import fs_extra_1 from "fs-extra";
import * as glob_1 from "glob";
import path_1 from "path";
import * as copy_resources_util_js_1 from "../../utils/copy-resources-util";
export async function copyByCopyOptions(e) {
  const t = (0, copy_resources_util_js_1.getCopyOptions)(e),
    {
      targets: o = [],
      ...r
    } = t;
  let s = [];
  Array.isArray(o) && o.length && (s = await getCopyTargets(o, r));
  for (const e of s) {
    const {
      dest: t,
      src: o
    } = e;
    fs_extra_1.existsSync(o) && copyFile(t, o);
  }
}
async function getCopyTargets(e, t) {
  const o = [];
  for (const r of e) {
    const {
        dest: e,
        rename: s,
        src: n,
        ...a
      } = r,
      c = glob_1.glob.sync(n, {
        nodir: true,
        ...t,
        ...a
      });
    if (c.length) for (const t of c) {
      const {
        base: r
      } = path_1.parse(t);
      o.push({
        src: t,
        dest: path_1.join(e, s ? renameTarget(r, s, t) : r),
        renamed: s
      });
    }
  }
  return o;
}
function renameTarget(e, t, o) {
  const r = path_1.parse(e);
  return "string" == typeof t ? t : t(r.name, r.ext.replace(".", ""), o);
}
function copyFile(e, t) {
  const o = path_1.dirname(e);
  fs_extra_1.ensureDirSync(o), fs_extra_1.copyFileSync(t, e);
}
