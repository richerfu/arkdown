/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/copy-resources.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as utils_js_1 from "./utils";
function renameDestFile(e) {
  return (t, s, a) => {
    a = (0, utils_js_1.toUnixPath)(a);
    const r = (0, utils_js_1.toUnixPath)(e);
    if (-1 !== a.indexOf(r)) {
      let e = a.replace(r, "");
      return e.startsWith("/") && (e = e.substring(1)), e;
    }
    return `${t}.${s}`;
  };
}
function formatCopyPatterns(e) {
  return e.forEach(e => {
    e.src = (0, utils_js_1.toUnixPath)(e.src), e.dest = (0, utils_js_1.toUnixPath)(e.dest);
  }), e;
}
export function getCopyPluginConfig(e, t) {
  const s = [],
    a = "../share",
    r = e.aceModuleRoot,
    i = e.aceModuleBuild || path_1.resolve(r, "build");
  s.push({
    src: path_1.resolve(r, "**/*"),
    dest: i,
    ignore: {
      ignored: e => /\.ets$|\.ts$|\.js$/.test(e.name)
    },
    onlyFiles: true,
    rename: renameDestFile(r)
  });
  const n = path_1.resolve(r, a);
  fs_1.existsSync(n) && s.push({
    src: path_1.resolve(n, "**/*"),
    dest: path_1.resolve(i, a),
    ignore: {
      ignored: e => /\.ets$|\.ts$|\.js$/.test(e.name)
    },
    onlyFiles: true,
    rename: renameDestFile(n)
  });
  "page" === (e.abilityType ?? "page") && e.aceManifestPath && fs_1.existsSync(e.aceManifestPath) && s.push({
    src: e.aceManifestPath,
    dest: i
  });
  const o = /ets_loader_ark$/.test(__dirname),
    l = e.cachePath ?? path_1.resolve(__dirname, "node_modules/.cache");
  return t && fs_1.existsSync(t) && !o && e.isPreview && s.push({
    src: path_1.resolve(__dirname, t),
    dest: path_1.resolve(__dirname, l)
  }), formatCopyPatterns(s);
}
