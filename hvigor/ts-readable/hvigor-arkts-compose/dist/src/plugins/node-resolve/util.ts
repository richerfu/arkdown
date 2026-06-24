/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/util.js
 */
import * as pluginutils_1 from "@rollup/pluginutils";
import * as path_1 from "path";
import * as fs_js_1 from "../../util/fs";
export function getPackageName(e) {
  if (e.startsWith(".") || e.startsWith("/")) return null;
  const t = e.split("/");
  return "@" === t[0][0] ? `${t[0]}/${t[1]}` : t[0];
}
export function getMainFields(e) {
  let t = ["module", "jsnext:main", "main"];
  if (e.mainFields && ({
    mainFields: t
  } = e), e.browser && -1 === t.indexOf("browser")) return ["browser"].concat(t);
  if (!t.length) throw new Error("Please ensure at least one `mainFields` value is specified.");
  return t;
}
function initPackageInfo(e, t, r) {
  return {
    packageJson: {
      ...e
    },
    packageJsonPath: t,
    root: r,
    resolvedMainField: "main",
    browserMappedMain: false,
    resolvedEntryPoint: ""
  };
}
function processBrowserField(e, t, r) {
  return Object.keys(e.browser).reduce((n, s) => {
    let a = e.browser[s];
    if (a && "." === a[0] && (a = (0, path_1.resolve)(t, a)), n[s] = a, "." === s[0]) {
      const e = (0, path_1.resolve)(t, s);
      n[e] = a, (0, path_1.extname)(s) || r.reduce((t, r) => (t[e + r] = t[s], t), n);
    }
    return n;
  }, {});
}
function initInternalPkgInfo(e, t, r, n, s, a, o) {
  const i = n && "object" == typeof e.browser && processBrowserField(e, s, a);
  return {
    cachedPkg: e,
    hasModuleSideEffects: () => null,
    hasPackageEntry: t || -1 !== r.indexOf("main"),
    packageBrowserField: i,
    packageInfo: o
  };
}
function updatePackageInfo(e, t, r, n, s) {
  const a = e.packageBrowserField;
  t && "object" == typeof r.browser && Object.prototype.hasOwnProperty.call(a, r.main) ? (n.resolvedEntryPoint = a[r.main], n.browserMappedMain = true) : (n.resolvedEntryPoint = (0, path_1.resolve)(s, r.main || "index.js"), n.browserMappedMain = false);
}
function processSideEffects(e, t, r) {
  const n = t.sideEffects;
  if ("boolean" == typeof n) e.hasModuleSideEffects = () => n;else if (Array.isArray(n)) {
    const t = n.map(e => e.includes("/") ? e : `**/${e}`);
    e.hasModuleSideEffects = (0, pluginutils_1.createFilter)(t, null, {
      resolve: r
    });
  }
}
export async function getPackageInfo(e) {
  const {
    cache: t,
    extensions: r,
    pkg: n,
    mainFields: s,
    preserveSymlinks: a,
    useBrowserOverrides: o,
    rootDir: i,
    ignoreSideEffectsForRoot: c
  } = e;
  let {
    pkgPath: l
  } = e;
  if (t.has(l)) return t.get(l);
  a || (l = await (0, fs_js_1.realpathFS)(l));
  const p = (0, path_1.dirname)(l),
    f = initPackageInfo(n, l, p);
  let u = false;
  for (const e of s) if ("string" == typeof n[e]) {
    f.resolvedMainField = e, n.main = n[e], u = true;
    break;
  }
  const d = initInternalPkgInfo(n, u, s, o, p, r, f);
  return updatePackageInfo(d, o, n, f, p), c && i === p || processSideEffects(d, n, p), t.set(l, d), d;
}
export function normalizeInput(e) {
  return Array.isArray(e) ? e : "object" == typeof e && null !== e ? Object.values(e) : [e];
}
