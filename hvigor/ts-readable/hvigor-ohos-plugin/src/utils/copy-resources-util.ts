/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/copy-resources-util.js
 */
import fs_1 from "fs";
import os_1 from "os";
import path_1 from "path";
import * as minimatch_1 from "minimatch";
function isCodeResourceIgnored(e, t, s) {
  if (e) return !e.some(e => (0, minimatch_1.minimatch)(path_1.resolve(s.path, s.name), e, {
    dot: true
  }));
  return /\.ets$|\.ts$|\.js$/.test(s.name) || t.some(e => (0, minimatch_1.minimatch)(path_1.resolve(s.path, s.name), e, {
    dot: true
  }));
}
export function getCopyOptions(e) {
  var t, s, a, o;
  null !== (t = e.isPreview) && undefined !== t || (e.isPreview = false);
  const r = [],
    i = "../share",
    n = e.aceModuleRoot,
    u = e.aceModuleBuild || path_1.resolve(n, "build"),
    l = null !== (s = e.copyCodeResourceExcludes) && undefined !== s ? s : [],
    p = e.copyCodeResourceIncludes,
    d = Array.from(new Set(["**/*.ets", "**/*.ts", "**/*.js", ...l]));
  r.push({
    src: path_1.resolve(n, "**/*"),
    dest: u,
    ignore: {
      ignored: e => isCodeResourceIgnored(p, d, e)
    },
    onlyFiles: true,
    rename: renameDestFile(n)
  });
  const c = path_1.resolve(n, i);
  fs_1.existsSync(c) && r.push({
    src: path_1.resolve(c, "**/*"),
    dest: path_1.resolve(u, i),
    ignore: {
      ignored: e => /\.ets$|\.ts$|\.js$/.test(e.name)
    },
    onlyFiles: true,
    rename: renameDestFile(c)
  });
  "page" === (null !== (a = e.abilityType) && undefined !== a ? a : "page") && e.aceManifestPath && fs_1.existsSync(e.aceManifestPath) && r.push({
    src: e.aceManifestPath,
    dest: u
  });
  const _ = /ets_loader_ark$/.test(__dirname),
    h = null !== (o = e.cachePath) && undefined !== o ? o : path_1.resolve(__dirname, "node_modules/.cache");
  return e.appResource && fs_1.existsSync(e.appResource) && !_ && e.isPreview && r.push({
    src: path_1.resolve(__dirname, e.appResource),
    dest: path_1.resolve(__dirname, h)
  }), {
    targets: formatCopyPatterns(r)
  };
}
function renameDestFile(e) {
  return (t, s, a) => {
    a = toUnixPath(a);
    const o = toUnixPath(e);
    if (-1 !== a.indexOf(o)) {
      let e = a.replace(o, "");
      return e.startsWith("/") && (e = e.substring(1)), e;
    }
    return `${t}.${s}`;
  };
}
function formatCopyPatterns(e) {
  return e.forEach(e => {
    e.src = toUnixPath(e.src), e.dest = toUnixPath(e.dest);
  }), e;
}
export function toUnixPath(e) {
  return "Windows_NT" === os_1.type() ? path_1.posix.join(...e.split(path_1.sep)) : e;
}
