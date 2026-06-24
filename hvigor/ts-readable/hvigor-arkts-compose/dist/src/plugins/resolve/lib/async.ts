/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/resolve/lib/async.js
 */
import fs_1 from "fs";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import path_1 from "path";
import * as fs_js_1 from "../../../util/fs";
import * as caller_js_1 from "./caller";
import * as homedir_js_1 from "./homedir";
import * as is_core_js_1 from "./is-core";
import * as node_modules_paths_js_1 from "./node-modules-paths";
import * as normalize_options_js_1 from "./normalize-options";
const homedir = (0, homedir_js_1.getHomedir)(),
  defaultPaths = function () {
    return [path_1.join(homedir, ".oh_modules"), path_1.join(homedir, ".node_libraries")];
  },
  defaultIsFile = async function (t, e) {
    try {
      const s = fs_1.statSync(t);
      return await e(null, s.isFile() || s.isFIFO());
    } catch (t) {
      return "ENOENT" === t.code || "ENOTDIR" === t.code ? e(null, false) : e(t);
    }
  },
  defaultIsDir = async function (t, e) {
    try {
      const s = fs_1.statSync(t);
      return await e(null, s.isDirectory());
    } catch (t) {
      return "ENOENT" === t.code || "ENOTDIR" === t.code ? e(null, false) : e(t);
    }
  },
  defaultRealpath = async function (t, e) {
    let s;
    try {
      return s = await (0, fs_js_1.realpathFS)(t), await e(null, s);
    } catch (i) {
      return i && "ENOENT" !== i.code ? e(i) : e(null, i ? t : s);
    }
  },
  maybeRealpath = async function (t, e, s, i) {
    return s && false === s.preserveSymlinks ? t(e, i) : i(null, e);
  },
  defaultReadPackage = async function (t, e, s) {
    return await t(e, async (t, e) => {
      if (t) return s(t);
      try {
        const t = (0, hvigor_common_1.parseJsonText)(e);
        return await s(null, t);
      } catch (t) {
        return s(null);
      }
    });
  },
  getPackageCandidates = function (t, e, s) {
    const i = (0, node_modules_paths_js_1.nodeModulesPaths)(e, s, t);
    for (let e = 0; e < i.length; e++) i[e] = path_1.join(i[e], t);
    return i;
  };
export async function resolve(t, e) {
  return new Resolver(t, e).resolve();
}
class Resolver {
  constructor(t, e) {
    this.onfile = async (t, e, s) => t || e || this.loadAsDirectory(this.res, async (t, e, s) => {
      if (t) return t;
      if (e) return maybeRealpath(this.realpath, e, this.opts, (t, e) => t || e);
      {
        const t = new Error(`Cannot find module '${this.x}' from '${this.parent}'`);
        return t.code = "MODULE_NOT_FOUND", t;
      }
    }), this.loadAsFile = async (t, e, s, i) => {
      let a = e,
        r = s;
      "function" == typeof a && (r = a, a = undefined);
      const o = [""].concat(this.extensions),
        n = async (t, e, s) => {
          if (0 === t.length) return r(null, undefined, s);
          const a = e + t[0],
            o = async (s, i) => s ? r(s) : i ? r(null, a, l) : n(t.slice(1), e, l),
            h = async (s, h, c) => {
              if (l = h, s) return r(s);
              if (c && l && this.opts.pathFilter) {
                const s = path_1.relative(c, a),
                  i = s.slice(0, s.length - t[0].length),
                  r = this.opts.pathFilter(l, e, i);
                if (r) return n([""].concat(this.extensions.slice()), path_1.resolve(c, r), l);
              }
              return this.isFile(a, o, {
                rootDir: this.rootDir,
                caseSensitiveCheck: this.caseSensitiveCheck,
                sdkModulesPath: this.sdkModulesPath,
                importer: this.importer,
                originImportPath: this.originImportPath,
                joinPackageJson: this.joinPackageJson,
                joinIndex: i?.joinIndex
              });
            };
          let l = s;
          return l ? h(null, l) : this.loadpkg(path_1.dirname(a), h);
        };
      return n(o, t, a);
    }, this.loadpkg = async (t, e) => "" === t || "/" === t || "win32" === process.platform && /^\w:[/\\]*$/.test(t) || /[/\\]oh_modules[/\\]*$/.test(t) ? e(null) : maybeRealpath(this.realpath, t, this.opts, async (s, i) => {
      if (s) return this.loadpkg(path_1.dirname(t), e);
      const a = path_1.join(i, "oh-package.json5");
      return this.isFile(a, async (s, i) => i ? this.readPackage(this.readFile, a, async (s, i) => {
        if (s) return e(s);
        let r = i;
        return r && this.opts.packageFilter && (r = await this.opts.packageFilter(r, a)), e(null, r, t);
      }) : this.loadpkg(path_1.dirname(t), e), {
        rootDir: this.rootDir,
        caseSensitiveCheck: this.caseSensitiveCheck,
        sdkModulesPath: this.sdkModulesPath,
        importer: this.importer,
        originImportPath: this.originImportPath,
        joinPackageJson: this.joinPackageJson
      });
    }), this.loadAsDirectory = async (t, e, s) => {
      let i = s,
        a = e;
      return "function" == typeof a && (i = a, a = this.opts.package), maybeRealpath(this.realpath, t, this.opts, async (e, s) => {
        if (e) return i(e);
        const r = path_1.join(s, "oh-package.json5");
        return this.isFile(r, this.getAsyncCallback(i, t, a, r), {
          rootDir: this.rootDir,
          caseSensitiveCheck: this.caseSensitiveCheck,
          sdkModulesPath: this.sdkModulesPath,
          importer: this.importer,
          originImportPath: this.originImportPath,
          joinPackageJson: this.joinPackageJson
        });
      });
    }, this.processDirs = async (t, e) => {
      if (0 === e.length) return t(null, undefined);
      const s = e[0],
        i = async (s, i, a) => s ? t(s) : i ? t(null, i, a) : this.processDirs(t, e.slice(1)),
        a = async (e, a, r) => e ? t(e) : a ? t(null, a, r) : this.loadAsDirectory(s, this.opts.package, i);
      return this.isDirectory(path_1.dirname(s), async (i, r) => i ? t(i) : r ? this.loadAsFile(s, this.opts.package, a) : this.processDirs(t, e.slice(1)), {
        rootDir: this.rootDir,
        caseSensitiveCheck: this.caseSensitiveCheck,
        sdkModulesPath: this.sdkModulesPath,
        importer: this.importer,
        originImportPath: this.originImportPath,
        joinPackageJson: this.joinPackageJson
      });
    }, this.loadNodeModules = async (t, e, s) => {
      const i = () => getPackageCandidates(t, e, this.opts);
      return this.processDirs(s, this.packageIterator ? this.packageIterator(t, e, i, this.opts) : i());
    }, this.x = t, this.opts = e ?? {};
  }
  async resolve() {
    if ("string" != typeof this.x) return new TypeError("Path must be a string.");
    if (this.opts = (0, normalize_options_js_1.normalizeOptions)(this.x, this.opts), this.isFile = this.opts.isFile || defaultIsFile, this.isDirectory = this.opts.isDirectory || defaultIsDir, this.readFile = this.opts.readFile, this.realpath = this.opts.realpath || defaultRealpath, this.readPackage = this.opts.readPackage || defaultReadPackage, this.opts.readFile && this.opts.readPackage) return new TypeError("`readFile` and `readPackage` are mutually exclusive.");
    this.packageIterator = this.opts.packageIterator, this.extensions = this.opts.extensions || [".js"], this.includeCoreModules = false !== this.opts.includeCoreModules, this.basedir = this.opts.basedir || path_1.dirname((0, caller_js_1.caller)()), this.parent = this.opts.filename || this.basedir, this.rootDir = this.opts.rootDir, this.caseSensitiveCheck = this.opts.caseSensitiveCheck ?? false, this.sdkModulesPath = this.opts.sdkModulesPath || [], this.importer = this.opts.importer, this.originImportPath = this.opts.originImportPath, this.joinPackageJson = this.opts.joinPackageJson ?? false, this.opts.paths = this.opts.paths || defaultPaths();
    const t = path_1.resolve(this.basedir);
    return maybeRealpath(this.realpath, t, this.opts, async (t, e) => t || this.init(e));
  }
  async init(t) {
    return /^(?:\.\.?(?:\/|$)|\/|([A-Za-z]:)?[/\\])/.test(this.x) ? (this.res = path_1.resolve(t, this.x), "." !== this.x && ".." !== this.x && "/" !== this.x.slice(-1) || (this.res += "/"), /\/$/.test(this.x) && this.res === t ? this.loadAsDirectory(this.res, this.opts.package, this.onfile) : this.loadAsFile(this.res, this.opts.package, this.onfile)) : this.includeCoreModules && (0, is_core_js_1.isCore)(this.x) ? this.x : this.loadNodeModules(this.x, t, async (t, e, s) => {
      if (t) return t;
      if (e) return maybeRealpath(this.realpath, e, this.opts, async (t, e) => t || e);
      {
        const t = new Error(`Cannot find module '${this.x}' from '${this.parent}'`);
        return t.code = "MODULE_NOT_FOUND", t;
      }
    });
  }
  isInterfaceHar(t) {
    return "InterfaceHar" === t?.packageType;
  }
  isByteCodeHar(t) {
    return true === t?.metadata?.byteCodeHar;
  }
  getAsyncCallback(t, e, s, i) {
    return async (a, r) => a ? t(a) : r ? this.readPackage(this.readFile, i, async (s, a) => {
      if (s) return t(s);
      let r = a;
      if (r && this.opts.packageFilter && (r = await this.opts.packageFilter(r, i)), r && r.main) {
        if ("string" != typeof r.main) {
          const e = new TypeError(`package “${r.name}” \`main\` must be a string`);
          return e.code = "INVALID_PACKAGE_MAIN", t(e);
        }
        return "." !== r.main && "./" !== r.main || (r.main = "index"), this.loadAsFile(path_1.resolve(e, r.main), r, async (s, i, a) => {
          if (s) return t(s);
          if (i) return t(null, i, a);
          if (!a) return this.loadAsFile(path_1.join(e, "index"), a, t, {
            joinIndex: true
          });
          const r = path_1.resolve(e, a.main);
          return this.loadAsDirectory(r, a, async (s, i, a) => s ? t(s) : i ? t(null, i, a) : this.loadAsFile(path_1.join(e, "index"), a, t, {
            joinIndex: true
          }));
        });
      }
      return (this.isInterfaceHar(r) || this.isByteCodeHar(r)) && r?.types ? this.loadAsFile(path_1.join(e, r.types), r, t, {
        joinIndex: true
      }) : this.loadAsFile(path_1.join(e, "/index"), r, t, {
        joinIndex: true
      });
    }) : this.loadAsFile(path_1.join(e, "index"), s, t, {
      joinIndex: true
    });
  }
}
