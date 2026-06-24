/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/plugins/terser-plugin.js
 */
import * as terser_1 from "terser";
export function terserPlugin(e = {}) {
  return {
    name: "terserPlugin",
    async renderChunk(r, t, s) {
      const o = {
          ...e,
          sourceMap: true === s.sourcemap || "string" == typeof s.sourcemap,
          module: "es" === s.format || e?.module,
          toplevel: "cjs" === s.format || e?.toplevel
        },
        n = await (0, terser_1.minify)(r, o),
        u = "string" == typeof n.map ? n.map : n.map?.mappings;
      return {
        code: n.code ?? "",
        map: u
      };
    }
  };
}
