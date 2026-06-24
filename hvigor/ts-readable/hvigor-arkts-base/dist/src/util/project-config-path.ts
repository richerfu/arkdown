/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/util/project-config-path.js
 */
import node_path_1 from "node:path";
import fs_extra_1 from "fs-extra";
const PROJECT_PATH_PLACEHOLDER = "%project_path%",
  SDK_UNDERLINE_PATH_PLACEHOLDER = "%sdk_underline_path%",
  SDK_NATIVE_UNDERLINE_PATH_PLACEHOLDER = "%sdk_native_underline_path%",
  CUSTOM_BUILD_CACHE_DIR = "%build_cache_dir%";
class ProjectConfigPath {
  init(e, t, r) {
    this.projectTopPath = e, this.sdkPath = t, this.sdkUnderlinePath = this.sdkPath?.replace(/\\/g, "_").replace(/\//g, "_"), this.sdkNativePath = fs_extra_1.realpathSync.native(this.sdkPath), this.sdkNativeUnderlinePath = this.sdkNativePath?.replace(/\\/g, "_").replace(/\//g, "_"), this.buildDir = node_path_1.normalize(r), this.buildCacheProjectDir = this.getBuildCacheProjectDir(e, r);
  }
  replacePluginsCachePath(e, t) {
    t ? this.handlePluginsCache(e, this.replaceToPlaceholder) : this.handlePluginsCache(e, this.replaceToNativePath);
  }
  replaceModuleCachePath(e, t) {
    if ((e.id.includes("%project_path%") || t) && undefined !== this.projectTopPath && (t ? this.handleModuleCache(e, this.replaceToPlaceholder) : this.handleModuleCache(e, this.replaceToNativePath), e.meta.tsProgram && e.meta.tsProgram.sourceFileToPackageName && e.meta.tsProgram.sourceFileToPackageName instanceof Map)) {
      const r = e.meta.tsProgram.sourceFileToPackageName,
        a = new Map();
      for (const e of r.keys()) t ? a.set(e.replace(this.projectTopPath.toLowerCase().replace(/\\/g, "/"), "%project_lower_path%"), r.get(e) ?? "") : a.set(e.replace("%project_lower_path%", this.projectTopPath.toLowerCase()).replace(/\\/g, "/"), r.get(e) ?? "");
      e.meta.tsProgram.sourceFileToPackageName = a;
    }
  }
  handlePluginsCache(e, t) {
    for (const r in e) for (const a in e[r]) {
      const o = t(a);
      o !== a && (e[r][o] = e[r][a], delete e[r][a]), e[r][o].forEach((a, c) => {
        "string" == typeof a ? e[r][o][c] = t(a) : "object" == typeof a ? (this.replaceStringProperties(a, t), e[r][o][c] = a) : e[r][o][c] = a;
      });
    }
  }
  handleModuleCache(e, t) {
    if (this.projectTopPath) {
      e.id = t(e.id);
      const r = [];
      e.dependencies?.forEach(e => {
        r.push(t(e));
      }), e.dependencies = r, e.meta.pkgPath = t(e.meta.pkgPath), e.meta.belongProjectPath = t(e.meta.belongProjectPath), e.meta.belongModulePath = t(e.meta.belongModulePath);
      const a = Object.keys(e.resolvedIds);
      for (const r of a) {
        const a = t(r);
        if (r !== a) {
          const t = e.resolvedIds[r];
          delete e.resolvedIds[r], e.resolvedIds[a] = t;
        }
        e.resolvedIds[a].id = t(e.resolvedIds[a].id);
      }
      e.transformFiles?.forEach(e => {
        "chunk" === e.type && (e.id = t(e.id));
      });
      const o = [];
      e.transformDependencies.forEach(e => {
        o.push(t(e));
      }), e.transformDependencies = o;
      const c = [];
      e.originalSourcemap?.sources.forEach(e => {
        c.push(t(e));
      }), e.originalSourcemap?.sources && (e.originalSourcemap.sources = c);
    }
    e.meta.commonjs && this.replaceStringProperties(e.meta.commonjs, t), this.replaceImportCache(e, t), this.replaceExportAllDeclarationCache(e, t);
  }
  replaceExportAllDeclarationCache(e, t) {
    e.cacheAst?.exportAllDeclarationCache && (e.cacheAst.exportAllDeclarationCache.forEach(e => {
      e.source && (e.source.raw = e.source.raw.replace(/\\\\/g, "\\"));
    }), this.replaceStringProperties(e.cacheAst?.exportAllDeclarationCache, t), e.cacheAst.exportAllDeclarationCache.forEach(e => {
      e.source && (e.source.raw = e.source.raw.replace(/\\/g, "\\\\").replace("\\\\", "\\"));
    }));
  }
  replaceImportCache(e, t) {
    e.cacheAst?.importCache && (e.cacheAst.importCache.forEach(e => {
      e.source && (e.source.raw = e.source.raw.replace(/\\\\/g, "\\"));
    }), this.replaceStringProperties(e.cacheAst?.importCache, t), e.cacheAst.importCache.forEach(e => {
      e.source && (e.source.raw = e.source.raw.replace(/\\/g, "\\\\").replace("\\\\", "\\"));
    }));
  }
  replaceStringProperties(e, t) {
    if ("object" == typeof e && null !== e) if (Array.isArray(e)) e.forEach((r, a) => {
      "string" == typeof r ? e[a] = t(e[a]) : "object" == typeof r && null !== r && this.replaceStringProperties(r, t);
    });else if (e instanceof Map) e.forEach((r, a) => {
      const o = t(a);
      o !== a && (e.set(o, r), e.delete(a)), "string" == typeof r ? e.set(o, t(r)) : "object" == typeof r && null !== r && this.replaceStringProperties(r, t);
    });else for (const r of Object.keys(e)) {
      const a = t(r);
      if (a !== r && (e[a] = e[r], delete e[r]), Object.prototype.hasOwnProperty.call(e, a)) {
        const r = e[a];
        "string" == typeof r ? e[a] = t(e[a]) : "object" == typeof r && null !== r && this.replaceStringProperties(r, t);
      }
    }
  }
  replaceToNativePath(e) {
    if (!e) return e;
    let t = projectConfigPath.projectTopPath ? e.replace("%project_path%", projectConfigPath.projectTopPath) : e;
    return t = projectConfigPath.sdkUnderlinePath ? t.replace("%sdk_underline_path%", projectConfigPath.sdkUnderlinePath) : t, t = projectConfigPath.sdkNativeUnderlinePath ? t.replace("%sdk_native_underline_path%", projectConfigPath.sdkNativeUnderlinePath) : t, t = projectConfigPath.sdkPath ? t.replace("%sdk_path%", projectConfigPath.sdkPath.replace(/\\/g, node_path_1.sep).replace(new RegExp("/", "g"), node_path_1.sep)) : t, t = projectConfigPath.buildCacheProjectDir ? t.replace("%build_cache_dir%", projectConfigPath.buildCacheProjectDir) : t, t;
  }
  replaceToPlaceholder(e) {
    if (!e) return e;
    let t = projectConfigPath.projectTopPath && projectConfigPath.buildDir && !e.startsWith(projectConfigPath.buildDir) ? e.replace(projectConfigPath.projectTopPath, "%project_path%") : e;
    return t = projectConfigPath.sdkUnderlinePath ? t.replace(projectConfigPath.sdkUnderlinePath, "%sdk_underline_path%") : t, t = projectConfigPath.sdkNativeUnderlinePath ? t.replace(projectConfigPath.sdkNativeUnderlinePath, "%sdk_native_underline_path%") : t, t = projectConfigPath.sdkPath ? t.replace(projectConfigPath.sdkPath.replace(/\\/g, node_path_1.sep).replace(new RegExp("/", "g"), node_path_1.sep), "%sdk_path%") : t, t = projectConfigPath.buildCacheProjectDir ? t.replace(projectConfigPath.buildCacheProjectDir, "%build_cache_dir%") : t, t;
  }
  getBuildCacheProjectDir(e, t) {
    const r = node_path_1.basename(e);
    return node_path_1.normalize(t.substring(0, t.indexOf(r) + r.length));
  }
}
export const projectConfigPath = new ProjectConfigPath();
