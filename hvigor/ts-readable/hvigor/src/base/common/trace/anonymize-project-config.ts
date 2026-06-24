/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/trace/anonymize-project-config.js
 */
export class AnonymizeProjectConfig {
  constructor() {
    this.PROJECT_CONFIG_KEYS = ["buildMode", "compileMode", "moduleType", "targetName", "packageManagerType", "isLocalTest", "needCoverageInsert", "debugLine", "compileSdkVersion", "compatibleSdkVersion", "originCompatibleSdkVersion", "etsLoaderVersion", "etsLoaderReleaseType", "aotCompileMode", "isCrossplatform", "bundleType", "apiVersion", "isFaMode", "deviceTypes", "integratedHsp", "branchElimination", "tsImportSendable", "resolveConflictMode", "useNativeResolver", "shouldEmitJs", "useTscResolve", "singleFileEmit", "executionMode", "isHarDeduplicateEnabled", "autoLazyImport", "allowEmptyBundleName", "reExportCheckMode", "usePathPlaceholder", "writeRollupCache", "isExpandImportEnabled", "ignoreCrossplatformCheck", "skipOhModulesLint", "enableStrictCheckOHModule", "useDeclarationFileSignature", "isPreloadSoEnabled", "strictMode.caseSensitiveCheck", "strictMode.useNormalizedOHMUrl", "strictMode.noExternalImportByPath", "runtimeOS", "sdkInfo", "useSourceHash", "isByteCodeHarOptimize", "copyCodeResourceEnable", "uiTransformOptimization"], this.projectConfigRaw = {};
  }
  anonymizeProjectConfig(e) {
    return e && (this.projectConfigRaw = e), this.getAnonymizedProjectConfigArkTS();
  }
  getAnonymizedProjectConfigArkTS() {
    const e = {};
    if (!this.projectConfigRaw) return e;
    for (let o = 0; o < this.PROJECT_CONFIG_KEYS.length; o++) {
      const t = this.PROJECT_CONFIG_KEYS[o];
      try {
        if (t.includes(".")) {
          const o = t.split(".");
          e[t] = this.projectConfigRaw[o[0]][o[1]];
        } else e[t] = this.projectConfigRaw[t];
      } catch (e) {}
    }
    return e;
  }
}
