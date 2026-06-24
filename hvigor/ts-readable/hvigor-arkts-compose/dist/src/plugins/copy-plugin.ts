/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/copy-plugin.js
 */
import fs_1 from "fs";
import * as glob_1 from "glob";
import path_1 from "path";
function renameTarget(e, t, r) {
  const n = path_1.parse(e);
  return "string" == typeof t ? t : t(n.name, n.ext.replace(".", ""), r);
}
async function getCopyTargets(e, t) {
  const r = [];
  for (const n of e) {
    const {
        dest: e,
        rename: s,
        src: o,
        ...a
      } = n,
      u = glob_1.glob.sync(o, {
        nodir: true,
        ...t,
        ...a
      });
    if (u.length) for (const t of u) {
      const {
        base: n
      } = path_1.parse(t);
      r.push({
        src: t,
        dest: path_1.join(e, s ? renameTarget(n, s, t) : n),
        renamed: s
      });
    }
  }
  return r;
}
function copyPlugin(e = {}) {
  const {
    copyOnce: t = false,
    hook: r = "buildEnd",
    targets: n = [],
    ...s
  } = e;
  let o = false;
  function a(e, t) {
    const r = path_1.dirname(e);
    fs_1.existsSync(r) || fs_1.mkdirSync(r, {
      recursive: true
    }), fs_1.copyFileSync(t, e);
  }
  return {
    name: "copyPlugin",
    [r]: async () => {
      if (t && o) return;
      let e = [];
      Array.isArray(n) && n.length && (e = await getCopyTargets(n, s));
      for (const t of e) {
        const {
          dest: e,
          src: r
        } = t;
        fs_1.existsSync(r) && a(e, r);
      }
      o = true;
    }
  };
}
export default copyPlugin;
