/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/resolve-import-specifiers.js
 */
import fs_1 from "fs";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as path_1 from "path";
import * as url_1 from "url";
import * as util_1 from "util";
import * as fs_js_1 from "../../util/fs";
import * as index_js_1 from "../resolve/index";
import * as resolve_package_exports_js_1 from "./package/resolve-package-exports";
import * as resolve_package_imports_js_1 from "./package/resolve-package-imports";
import * as utils_js_1 from "./package/utils";
import * as cache_js_1 from "./cache";
import * as util_js_1 from "./util";
const readFile = (0, util_1.promisify)(fs_1.readFile);
async function getPackageJson(e, r, s, i) {
  if (e) {
    const s = await (0, utils_js_1.findPackageJson)(e, i);
    if (s && s.pkgJson.name === r) return s;
  }
  try {
    const e = await (0, index_js_1.resolve)(`${r}/oh-package.json5`, s);
    return {
      pkgJsonPath: e,
      pkgJson: (0, hvigor_common_1.parseJsonText)(await readFile(e, "utf-8")),
      pkgPath: (0, path_1.dirname)(e)
    };
  } catch (e) {
    return null;
  }
}
async function resolveWithExportMap({
  baseDir: e,
  exportConditions: r,
  extensions: s,
  ignoreSideEffectsForRoot: i,
  importer: o,
  importSpecifier: t,
  moduleDirectories: a,
  modulePaths: n,
  mainFields: c,
  packageInfoCache: l,
  preserveSymlinks: d,
  rootDir: p,
  useBrowserOverrides: u,
  caseSensitiveCheck: h,
  sdkModulesPath: f
}) {
  if (t.startsWith("#")) return numberHandler(t, o, a, r, l, s, c, d, u, e, n, p, h, f);
  const k = (0, util_js_1.getPackageName)(t);
  if (k) {
    let m,
      _ = () => null,
      g = true,
      v = false;
    const S = async (e, r) => {
        const o = await (0, util_js_1.getPackageInfo)({
          cache: l,
          extensions: s,
          ignoreSideEffectsForRoot: i,
          mainFields: c,
          pkg: e,
          pkgPath: r,
          preserveSymlinks: d,
          rootDir: p,
          useBrowserOverrides: u
        });
        return {
          packageInfo: m,
          hasModuleSideEffects: _,
          hasPackageEntry: g,
          packageBrowserField: v
        } = o, o.cachedPkg;
      },
      P = {
        basedir: e,
        extensions: s,
        includeCoreModules: false,
        isDirectory: cache_js_1.isDirCached,
        isFile: cache_js_1.isFileCached,
        moduleDirectory: a,
        packageFilter: S,
        paths: n,
        preserveSymlinks: d,
        readFile: cache_js_1.readCachedFile
      },
      D = await getPackageJson(o, k, {
        ...P,
        rootDir: p,
        caseSensitiveCheck: h,
        sdkModulesPath: f,
        importer: o,
        originImportPath: `${k}/oh-package.json5`,
        joinPackageJson: true
      }, a);
    if (D && D.pkgJson.exports) {
      const {
          pkgJson: e,
          pkgJsonPath: s
        } = D,
        i = k === t ? "." : `.${t.substring(k.length)}`,
        n = s.replace("oh-package.json5", ""),
        c = {
          conditions: r,
          importer: o,
          importSpecifier: t,
          moduleDirs: a,
          pkgJsonPath: s,
          pkgURL: (0, url_1.pathToFileURL)(n)
        },
        l = await (0, resolve_package_exports_js_1.resolvePackageExports)(c, i, e.exports),
        p = (0, url_1.fileURLToPath)(l);
      if (p) return {
        location: d ? p : await (0, fs_js_1.resolveSymlink)(p),
        hasModuleSideEffects: _,
        hasPackageEntry: g,
        packageBrowserField: v,
        packageInfo: m
      };
    }
  }
  return null;
}
async function numberHandler(e, r, s, i, o, t, a, n, c, l, d, p, u, h) {
  const f = await (0, resolve_package_imports_js_1.resolvePackageImports)({
      importSpecifier: e,
      importer: r,
      moduleDirs: s,
      conditions: i,
      resolveId: e => resolveIdClassic({
        baseDir: l,
        extensions: t,
        importSpecifier: e,
        mainFields: a,
        moduleDirectories: s,
        modulePaths: d,
        packageInfoCache: o,
        preserveSymlinks: n,
        useBrowserOverrides: c,
        rootDir: p,
        caseSensitiveCheck: u,
        sdkModulesPath: h
      })
    }),
    k = (0, url_1.fileURLToPath)(f);
  return {
    location: n ? k : await (0, fs_js_1.resolveSymlink)(k),
    hasModuleSideEffects: () => null,
    hasPackageEntry: true,
    packageBrowserField: false,
    packageInfo: undefined
  };
}
export async function resolveImportSpecifiers({
  baseDir: e,
  exportConditions: r,
  extensions: s,
  ignoreSideEffectsForRoot: i,
  importer: o,
  importSpecifierList: t,
  mainFields: a,
  moduleDirectories: n,
  modulePaths: c,
  packageInfoCache: l,
  preserveSymlinks: d,
  rootDir: p,
  useBrowserOverrides: u,
  warn: h,
  caseSensitiveCheck: f,
  sdkModulesPath: k
}) {
  try {
    const h = await resolveWithExportMap({
      baseDir: e,
      exportConditions: r,
      extensions: s,
      ignoreSideEffectsForRoot: i,
      importer: o,
      importSpecifier: t[0],
      mainFields: a,
      moduleDirectories: n,
      modulePaths: c,
      packageInfoCache: l,
      preserveSymlinks: d,
      rootDir: p,
      useBrowserOverrides: u,
      caseSensitiveCheck: f,
      sdkModulesPath: k
    });
    if (h) return h;
  } catch (e) {
    if (e instanceof utils_js_1.ResolveError) return h(e), null;
    throw e;
  }
  return resolveWithClassic({
    baseDir: e,
    exportConditions: r,
    extensions: s,
    ignoreSideEffectsForRoot: i,
    importer: o,
    importSpecifierList: t,
    mainFields: a,
    moduleDirectories: n,
    modulePaths: c,
    packageInfoCache: l,
    preserveSymlinks: d,
    rootDir: p,
    useBrowserOverrides: u,
    warn: h,
    caseSensitiveCheck: f,
    sdkModulesPath: k
  });
}
async function resolveIdClassic({
  baseDir: e,
  extensions: r,
  ignoreSideEffectsForRoot: s,
  importSpecifier: i,
  mainFields: o,
  moduleDirectories: t,
  modulePaths: a,
  packageInfoCache: n,
  preserveSymlinks: c,
  rootDir: l,
  useBrowserOverrides: d,
  caseSensitiveCheck: p,
  sdkModulesPath: u,
  importer: h
}) {
  let f,
    k = () => null,
    m = true,
    _ = false;
  const g = {
      basedir: e,
      extensions: r,
      includeCoreModules: false,
      isDirectory: cache_js_1.isDirCached,
      isFile: cache_js_1.isFileCached,
      moduleDirectory: t,
      packageFilter: async (e, i) => {
        const t = await (0, util_js_1.getPackageInfo)({
          cache: n,
          extensions: r,
          ignoreSideEffectsForRoot: s,
          mainFields: o,
          pkg: e,
          pkgPath: i,
          preserveSymlinks: c,
          rootDir: l,
          useBrowserOverrides: d
        });
        return {
          packageInfo: f,
          hasModuleSideEffects: k,
          hasPackageEntry: m,
          packageBrowserField: _
        } = t, t.cachedPkg;
      },
      paths: a,
      preserveSymlinks: c,
      readFile: cache_js_1.readCachedFile
    },
    v = await (0, index_js_1.resolve)(i, {
      ...g,
      rootDir: l,
      caseSensitiveCheck: p,
      sdkModulesPath: u,
      importer: h,
      originImportPath: i
    });
  if (v instanceof Error) {
    if ("MODULE_NOT_FOUND" !== v.code) throw v;
    return null;
  }
  return {
    location: c ? v : await (0, fs_js_1.resolveSymlink)(v),
    hasModuleSideEffects: k,
    hasPackageEntry: m,
    packageBrowserField: _,
    packageInfo: f
  };
}
async function resolveWithClassic({
  baseDir: e,
  exportConditions: r,
  extensions: s,
  ignoreSideEffectsForRoot: i,
  importer: o,
  importSpecifierList: t,
  mainFields: a,
  moduleDirectories: n,
  modulePaths: c,
  packageInfoCache: l,
  preserveSymlinks: d,
  rootDir: p,
  useBrowserOverrides: u,
  warn: h,
  caseSensitiveCheck: f,
  sdkModulesPath: k
}) {
  for (let m = 0; m < t.length; m++) {
    const _ = await resolveIdClassic({
      baseDir: e,
      exportConditions: r,
      extensions: s,
      ignoreSideEffectsForRoot: i,
      importer: o,
      importSpecifier: t[m],
      mainFields: a,
      moduleDirectories: n,
      modulePaths: c,
      packageInfoCache: l,
      preserveSymlinks: d,
      rootDir: p,
      useBrowserOverrides: u,
      warn: h,
      caseSensitiveCheck: f,
      sdkModulesPath: k
    });
    if (_) return {
      ..._,
      resolvedSpecifierIndex: m
    };
  }
  return null;
}
