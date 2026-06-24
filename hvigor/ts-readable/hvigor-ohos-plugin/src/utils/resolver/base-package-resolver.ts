/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/resolver/base-package-resolver.js
 */
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as common_const_1 from "../../const/common-const";
import * as oh_package_loader_1 from "../loader/file/oh-package-loader";
import * as resolver_cache_js_1 from "./resolver-cache";
const ROOT_PATH_REGEX = /^([A-Za-z]:)?[\\/]+/;
export class BasePackageResolver {
  constructor(e, a) {
    this.packageFileWord = e, this.moduleDirWord = a;
  }
  resolvePackagePath(e, a, r) {
    if (a.includes(common_const_1.CommonConst.DEPENDENCYMAP) && (resolver_cache_js_1.ResolverCache.get(path_1.resolve(a)) ? a = resolver_cache_js_1.ResolverCache.get(path_1.resolve(a)) : (oh_package_loader_1.ohPackageLoader.OhPackagePathMap.forEach((e, a) => {
      resolver_cache_js_1.ResolverCache.set(path_1.dirname(e), path_1.dirname(a));
    }), a = resolver_cache_js_1.ResolverCache.get(path_1.resolve(a)) || a)), resolver_cache_js_1.ResolverCache.has(`${e}\0${a}\0${this.packageFileWord}`)) return resolver_cache_js_1.ResolverCache.get(`${e}\0${a}\0${this.packageFileWord}`);
    const t = path_1.resolve(a),
      s = ROOT_PATH_REGEX.test(t) ? ROOT_PATH_REGEX.exec(t)[0] : path_1.resolve("/");
    let o = t;
    for (; s !== o;) {
      path_1.basename(o).toLowerCase() === this.moduleDirWord.toLowerCase() && (o = path_1.dirname(o));
      const t = path_1.resolve(o, this.moduleDirWord, e);
      if (fs_extra_1.pathExistsSync(t)) {
        const r = path_1.resolve(fs_extra_1.realpathSync.native(t), this.packageFileWord);
        return resolver_cache_js_1.ResolverCache.set(`${e}\0${a}\0${this.packageFileWord}`, r), r;
      }
      try {
        fs_extra_1.lstatSync(t).isSymbolicLink() && (null == r || r.warn("Please confirm the symbolic link is valid: ", t));
      } catch (e) {}
      o = path_1.dirname(o);
    }
  }
}
