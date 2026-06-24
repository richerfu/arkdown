/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/delete-plugin.js
 */
import fs_1 from "fs";
function delPlugin(e = {}) {
  const {
    hook: t = "buildStart",
    runOnce: r = false,
    targets: u = []
  } = e;
  let s = false;
  return {
    name: "delete",
    [t]: async () => {
      r && s || (u.forEach(e => {
        fs_1.existsSync(e) && fs_1.rmSync(e, {
          recursive: true
        });
      }), s = true);
    }
  };
}
export default delPlugin;
