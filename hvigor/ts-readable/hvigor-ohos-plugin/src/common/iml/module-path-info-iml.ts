/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/iml/module-path-info-iml.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as code_type_enum_js_1 from "../../enum/code-type-enum";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as task_util_js_1 from "../../utils/task-util";
export class ModulePathInfoIml {
  constructor(t, e, r, i = build_directory_const_js_1.BuildDirConst.BUILD_ROOT) {
    this._curModule = t, this._targetName = e, this._modulePath = t.getProjectDir(), this._productName = r, this._buildRoot = i;
  }
  getPatchDir() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.PATCH, this._targetName);
  }
  getObfuscationExportRulePath() {
    return path_1.resolve(this.getModuleBuildIntermediates(), "obfuscation", this._targetName, common_const_js_1.CommonConst.OBFUSCATION_TXT);
  }
  getIntermediatesJavaDir() {
    return path_1.resolve(this.getModuleBuildIntermediates(), "java", this._targetName);
  }
  getIntermediatesClassDir() {
    return path_1.resolve(this.getModuleBuildIntermediates(), "class", this._targetName);
  }
  getIntermediatesDexDir() {
    return path_1.resolve(this.getModuleBuildIntermediates(), "dex", this._targetName);
  }
  getIntermediatesShellClassDir() {
    return path_1.resolve(this.getModuleShellDir(), this.getBuildRoot(), "class");
  }
  getIntermediatesShellDexDir() {
    return path_1.resolve(this.getModuleShellDir(), this.getBuildRoot(), "dex");
  }
  getIntermediatesShellJavaDir() {
    return path_1.resolve(this.getShellSrcMain(), "java");
  }
  getIntermediatesApkDir() {
    return path_1.resolve(this.getModuleShellDir(), this.getBuildRoot(), "apk");
  }
  getBuildRoot() {
    return this._buildRoot;
  }
  getModuleBuildPath(t = true) {
    return path_1.resolve(inject_util_js_1.InjectUtil.getBuildCacheParentDir(this._modulePath, path_1.join(hvigor_1.hvigorCore.getProject().getName(), this._modulePath.substring(hvigor_1.hvigorCore.getProject().getNodeDir().length)), t), this._buildRoot);
  }
  getModuleSrcPath() {
    return path_1.resolve(this._modulePath, build_directory_const_js_1.BuildDirConst.SRC);
  }
  getModuleSrcMainPath() {
    return path_1.resolve(this._modulePath, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN);
  }
  getModuleSrcMockPath() {
    return path_1.resolve(this._modulePath, build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MOCK);
  }
  getModulePath() {
    return this._modulePath;
  }
  getModuleSrcMainResourceRawfileArkdataPath() {
    return path_1.resolve(this.getModuleSrcMainPath(), build_directory_const_js_1.BuildDirConst.RESOURCES, build_directory_const_js_1.BuildDirConst.RAW_FILE, build_directory_const_js_1.BuildDirConst.ARK_DATA, build_directory_const_js_1.BuildDirConst.SCHEMA);
  }
  getModuleSrcMainResourceRawfileUtdPath() {
    return path_1.resolve(this.getModuleSrcMainPath(), build_directory_const_js_1.BuildDirConst.RESOURCES, build_directory_const_js_1.BuildDirConst.RAW_FILE, build_directory_const_js_1.BuildDirConst.ARK_DATA, build_directory_const_js_1.BuildDirConst.UTD);
  }
  getModuleSrcMainResourceBaseProfilePath() {
    return path_1.resolve(this.getModuleSrcMainPath(), build_directory_const_js_1.BuildDirConst.RESOURCES_BASE_PROFILE_DIR);
  }
  getModulePreviewPath() {
    return path_1.resolve(this._modulePath, build_directory_const_js_1.BuildDirConst.PREVIEW_BUILD_PATH);
  }
  getModuleProductPath(t = true) {
    return path_1.resolve(this.getModuleBuildPath(t), this._productName);
  }
  getModuleTestPath() {
    return path_1.resolve(this._modulePath, build_directory_const_js_1.BuildDirConst.TEST_BUILD_PATH);
  }
  getModuleTestIntermediatesPath() {
    return path_1.resolve(this.getModuleTestPath(), this._productName, build_directory_const_js_1.BuildDirConst.INTERMEDIATES);
  }
  getModuleTestIntermediatesSrcOhosTestPath() {
    return path_1.resolve(this.getModuleTestIntermediatesPath(), "src", common_const_js_1.CommonConst.OHOS_TEST);
  }
  getOhosTestIntermediatesSrcResource() {
    return path_1.resolve(this.getModuleTestIntermediatesSrcOhosTestPath(), "resources");
  }
  getOhosTestIntermediatesSrcEts() {
    return path_1.resolve(this.getModuleTestIntermediatesSrcOhosTestPath(), "ets");
  }
  getModulePreviewProductPath() {
    return path_1.resolve(this.getModulePreviewPath(), this._productName);
  }
  getModulePreviewIntermediatesResPath() {
    return path_1.resolve(this.getModulePreviewProductPath(), build_directory_const_js_1.BuildDirConst.INTERMEDIATES, build_directory_const_js_1.BuildDirConst.INTERMEDIATES_RES, this._targetName);
  }
  getModulePreviewIntermediatesProfilePath() {
    return path_1.resolve(this.getModulePreviewIntermediatesResPath(), build_directory_const_js_1.BuildDirConst.RESOURCES_BASE_PROFILE_DIR);
  }
  getModuleBuildIntermediates() {
    return path_1.resolve(this.getModuleProductPath(), build_directory_const_js_1.BuildDirConst.INTERMEDIATES);
  }
  getModuleBuildGenerated() {
    return path_1.resolve(this.getModuleProductPath(), build_directory_const_js_1.BuildDirConst.GENERATED);
  }
  getModulePreviewBuildGenerated() {
    return path_1.resolve(this.getModulePreviewProductPath(), build_directory_const_js_1.BuildDirConst.GENERATED);
  }
  getModuleBuildOutputPath() {
    return path_1.resolve(this.getModuleProductPath(false), build_directory_const_js_1.BuildDirConst.BUILD_OUTPUTS, this._targetName);
  }
  getModuleBuildOhosTestOutputPath() {
    return path_1.resolve(this.getModuleProductPath(), build_directory_const_js_1.BuildDirConst.BUILD_OUTPUTS, common_const_js_1.CommonConst.OHOS_TEST);
  }
  getIntermediatesMergeRes() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.INTERMEDIATES_MERGE_RES, this._targetName);
  }
  getIntermediatesPacJsonDir() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.PAC_DIR, this._targetName);
  }
  getIntermediatesPacJsonPath() {
    return path_1.resolve(this.getIntermediatesPacJsonDir(), common_const_js_1.CommonConst.PAC_JSON);
  }
  getPacJsonPath() {
    return path_1.resolve(this.getModulePath(), common_const_js_1.CommonConst.PAC_JSON);
  }
  getIntermediatesMergeFile() {
    return path_1.resolve(this.getIntermediatesMergeRes(), build_directory_const_js_1.BuildArtifactConst.MERGE_RES_FILE_INDEX);
  }
  getIntermediatesMergeProfile() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.INTERMEDIATES_MERGE_PROFILE, this._targetName, common_const_js_1.CommonConst.MODULE_JSON);
  }
  getIntermediatesProcessProfileDir() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.INTERMEDIATES_PROCESS_PROFILE, this._targetName);
  }
  getGenerateSourceR() {
    return path_1.resolve(this.getModuleBuildGenerated(), build_directory_const_js_1.BuildDirConst.R, this._targetName);
  }
  getResourceTableTs() {
    return path_1.resolve(this.getGenerateSourceR(), build_directory_const_js_1.BuildArtifactConst.RESOURCE_TABLE_TS);
  }
  getGeneratePmDir() {
    return path_1.resolve(this.getModuleBuildGenerated(), build_directory_const_js_1.BuildDirConst.PM_DIR, this._targetName);
  }
  getGenerateBuildProfilePath(t) {
    return path_1.resolve(this.getGenerateBuildProfileDir(), t, build_directory_const_js_1.BuildArtifactConst.BUILD_PROFILE_FILE);
  }
  getHarGenerateBuildProfilePath() {
    return path_1.resolve(this._modulePath, build_directory_const_js_1.BuildArtifactConst.BUILD_PROFILE_FILE);
  }
  getGenerateModuleInfoPath() {
    return path_1.resolve(this.getGenerateBuildProfileDir(), this._targetName, build_directory_const_js_1.BuildArtifactConst.MODULE_INFO_FILE);
  }
  getGenerateBuildProfileDir() {
    return path_1.resolve(this.getModuleBuildGenerated(), build_directory_const_js_1.BuildArtifactConst.PROFILE_DIR);
  }
  getPreviewGenerateSourceR() {
    return path_1.resolve(this.getModulePreviewBuildGenerated(), build_directory_const_js_1.BuildDirConst.R, this._targetName);
  }
  getIntermediatesProcessProfile() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.INTERMEDIATES_PROCESS_PROFILE, this._targetName, common_const_js_1.CommonConst.MODULE_JSON);
  }
  getIntermediatesArkModuleJsonPath() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.INTERMEDIATES_RES, this._targetName, common_const_js_1.CommonConst.ARK_MODULE_JSON);
  }
  getIntermediatesProcessLegacyProfile() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.INTERMEDIATES_PROCESS_PROFILE, this._targetName, common_const_js_1.CommonConst.CONFIG_JSON);
  }
  getIntermediatesRes() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.INTERMEDIATES_RES, this._targetName);
  }
  getIntermediatesResDeduplicate() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.INTERMEDIATES_RES_DEDUPLICATE, this._targetName);
  }
  getDeduplicateResConfigPath() {
    return path_1.resolve(this.getIntermediatesResDeduplicate(), common_const_js_1.CommonConst.RES_CONFIG_FILE);
  }
  getIntermediatesOhosTestResourceStr() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.INTERMEDIATES_RES, common_const_js_1.CommonConst.OHOS_TEST, build_directory_const_js_1.BuildDirConst.RESOURCE_STR);
  }
  getIntermediatesMockConfigJsonFile() {
    return path_1.resolve(this.getIntermediatesRes(), build_directory_const_js_1.BuildArtifactConst.MOCK_CONFIG_JSON);
  }
  getIntermediatesResourcesRawFileMockDir() {
    return path_1.resolve(this.getIntermediatesRes(), build_directory_const_js_1.BuildDirConst.RESOURCES, build_directory_const_js_1.BuildDirConst.RAW_FILE, build_directory_const_js_1.BuildDirConst.MOCK);
  }
  getIntermediatesResProfilePath() {
    return path_1.resolve(this.getIntermediatesRes(), build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES, build_directory_const_js_1.BuildDirConst.BASE, build_directory_const_js_1.BuildDirConst.PROFILE);
  }
  getIntermediatesResDeduplicateProfilePath() {
    return path_1.resolve(this.getIntermediatesResDeduplicate(), build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES, build_directory_const_js_1.BuildDirConst.BASE, build_directory_const_js_1.BuildDirConst.PROFILE);
  }
  getIntermediatesResArkDataPath() {
    return path_1.resolve(this.getIntermediatesRes(), build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES, build_directory_const_js_1.BuildDirConst.RAW_FILE, build_directory_const_js_1.BuildDirConst.ARK_DATA, build_directory_const_js_1.BuildDirConst.SCHEMA);
  }
  getIntermediatesFaAssetsPath(t) {
    return path_1.resolve(this.getModuleBuildIntermediates(), `${build_directory_const_js_1.BuildDirConst.JS_BUILD_ASSETS}_${this._curModule.getCompileMode(t)}`, this._targetName);
  }
  getInterMediatesLoaderOutPath() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.LOADER_OUT, this._targetName);
  }
  getInterMediatesSourceMapDirPath() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.SOURCEMAP_INTERMEDIATE_DIR, this._targetName);
  }
  getSourceMapIntermediatePath() {
    const t = path_1.resolve(this.getInterMediatesSourceMapDirPath(), build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP);
    return fs_extra_1.existsSync(t) ? t : "";
  }
  getSourceMapOriginPath(t) {
    var e;
    const r = t.getModuleService().getModuleModel().isHarModule() ? "HarCompileArkTS" : "CompileArkTS";
    return (null === (e = t.getBuildOption()) || undefined === e ? undefined : e.debuggable) ? path_1.resolve(this.getInterMediatesLoaderOutPath(), code_type_enum_js_1.CodeType.ETS, build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP) : path_1.resolve(this.getModuleBuildCachePath(), `${this._targetName}@${r}`, "esmodule", "release", build_directory_const_js_1.BuildArtifactConst.SOURCEMAPS_MAP);
  }
  getInterMediatesAssetsPath() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.JS_BUILD_ASSETS, this._targetName);
  }
  getInterMediatesLoaderOutLitePath() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.LOADER_OUT_LITE, this._targetName);
  }
  getIntermediatesLoaderPath() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.LOADER, this._targetName);
  }
  getIntermediatesHarDuplicatePath() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.HAR_DUPLICATE, this._targetName);
  }
  getIntermediatesIdDefinedPath() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.ID_DEFINED, this._targetName);
  }
  getIntermediatesIdDefinedJsonFile() {
    return path_1.resolve(this.getIntermediatesIdDefinedPath(), common_const_js_1.CommonConst.ID_DEFINED_JSON);
  }
  getIntermediatesEtsTgzPath(t) {
    return path_1.resolve(this.getInterMediatesLoaderOutPath(), build_directory_const_js_1.BuildDirConst.DECLARE_FILE_OUTPUT, t);
  }
  getIntermediatesPkgContextInfoPath() {
    return path_1.resolve(this.getIntermediatesLoaderPath(), common_const_js_1.CommonConst.PKG_CONTEXT_INFO_JSON);
  }
  getIntermediatesPkgSdkInfoPath() {
    return path_1.resolve(this.getIntermediatesLoaderPath(), common_const_js_1.CommonConst.PKG_SDK_INFO_JSON);
  }
  getIntermediatesApPath() {
    return path_1.resolve(this.getModuleBuildIntermediates(), "ap");
  }
  getModuleBuildCachePath() {
    return path_1.resolve(this.getModuleProductPath(), build_directory_const_js_1.BuildDirConst.BUILD_CACHE, this._targetName);
  }
  getIntermediatesMergeLegacyProfile() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.INTERMEDIATES_MERGE_PROFILE, this._targetName, common_const_js_1.CommonConst.CONFIG_JSON);
  }
  getIntermediatesLegacyManifestJson() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.LEGACY_MANIFEST, this._targetName);
  }
  getIntermediatesPackInfoDir() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.LEGACY_MANIFEST, this._targetName);
  }
  getIntermediatesProcessLibs() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.LIBS, this._targetName);
  }
  getIntermediatesDoNativeBinxo() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.BINXO, this._targetName);
  }
  getIntermediatesStrippedLibsDir() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.STRIPPED_NATIVE_LIBS, this._targetName);
  }
  getIntermediatesPatch() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.PATCH, this._targetName);
  }
  getIntermediatesSysCap() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.SYSCAP, this._targetName);
  }
  getIntermediatesRouterMap(t) {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.INTERMEDIATES_ROUTER_MAP_TEMP_DIR, this._targetName, `${t}.json`);
  }
  getIntermediatesShareConfig(t) {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.INTERMEDIATES_SHARE_CONFIG_TEMP_DIR, this._targetName, `${t}.json`);
  }
  getIntermediatesStartupPath(t) {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.INTERMEDIATES_STARTUP_TEMP_DIR, this._targetName, `${null != t ? t : build_directory_const_js_1.BuildDirConst.DEFAULT_STARTUP_CONFIG_FILE_NAME}.json`);
  }
  getCppOutputDir() {
    return path_1.resolve((0, hvigor_common_1.isCI)() ? inject_util_js_1.InjectUtil.getBuildCacheParentDir(this._modulePath, path_1.join(hvigor_1.hvigorCore.getProject().getName(), this._modulePath.substring(hvigor_1.hvigorCore.getProject().getNodeDir().length))) : this._modulePath, build_directory_const_js_1.BuildDirConst.CPP_OUTPUT);
  }
  getNinjaWorkDir() {
    return path_1.resolve(this.getCppOutputDir(), this._productName, this._targetName, (0, task_util_js_1.getBuildModeName)());
  }
  getModuleShellDir() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.SHELL, this._targetName);
  }
  getShellResourceDir() {
    return path_1.resolve(this.getShellSrcMain(), build_directory_const_js_1.BuildDirConst.INTERMEDIATES_RES);
  }
  getShellSrcMain() {
    return path_1.resolve(this.getModuleShellDir(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN);
  }
  getShellBuildResourceDir() {
    return path_1.resolve(this.getModuleShellDir(), this.getBuildRoot(), build_directory_const_js_1.BuildDirConst.INTERMEDIATES_RES);
  }
  getIntermediatesCppOutPut() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.CMAKE, this._targetName, build_directory_const_js_1.BuildDirConst.OBJ);
  }
  getIntermediatesBuildInfo() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.BUILD_INFO, this._targetName, "meta");
  }
  getIntermediatesLiteBinSource() {
    return path_1.resolve(this.getModuleBuildIntermediates(), "lite_res", this._targetName);
  }
  getModuleBinOutput() {
    return path_1.resolve(this.getModuleBuildOutputPath(), "bin");
  }
  getIntermediatesLiteSource() {
    return path_1.resolve(this.getModuleBuildIntermediates(), "lite_source", this._targetName);
  }
  getIntermediatesTemp() {
    return path_1.resolve(this.getModuleBuildIntermediates(), this._targetName, build_directory_const_js_1.BuildDirConst.TEMP_DIR);
  }
  getBuildConfigPath(t) {
    return path_1.resolve(t ? this.getModulePreviewPath() : this.getModuleBuildPath(), "config", "buildConfig.json");
  }
  getIntermediatesOutputMetadata() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.HAP_METADATA, this._targetName, "output_metadata.json");
  }
  getTestBuildConfigPath() {
    return path_1.resolve(this.getModuleTestPath(), "config", "buildConfig.json");
  }
  getIntermediatesHspOutputMetadata() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.HSP_METADATA, this._targetName, build_directory_const_js_1.BuildArtifactConst.OUTPUT_METADATA_JSON);
  }
  getFromConfigPathByTargetName(t, e, r) {
    if (r) return path_1.resolve(t.getSourceSetByTargetName(this._targetName).getTargetResPath(), "base", "profile", `${r}.json`);
  }
  getSyscapJsonPathInfo(t) {
    const e = t.getSourceRootByTargetName(this._targetName);
    return path_1.resolve(e, common_const_js_1.CommonConst.SYSCAP_JSON);
  }
  getPreviewIntermediatesResModuleJsonPath() {
    return path_1.resolve(this.getModulePreviewIntermediatesResPath(), common_const_js_1.CommonConst.MODULE_JSON);
  }
  getPreviewIntermediatesMainPagesJsonPath() {
    return path_1.resolve(this.getModulePreviewIntermediatesProfilePath(), build_directory_const_js_1.BuildArtifactConst.MAIN_PAGES_JSON);
  }
  getUnitTestTemplatePath() {
    return path_1.resolve(this.getModuleBuildPath(), "testability");
  }
  getIntermediatesCangjieOutPut() {
    return path_1.resolve(this.getModuleBuildIntermediates(), "cj", "extend_libs", this._targetName);
  }
  getIntegratedHspOutputPath() {
    return path_1.resolve(this.getModuleBuildOutputPath(), build_directory_const_js_1.BuildDirConst.INTEGRATED_HSP);
  }
  getDefaultSourceMapPath() {
    return path_1.resolve(this.getModuleBuildPath(), this._productName, build_directory_const_js_1.BuildDirConst.BUILD_OUTPUTS, this._targetName, build_directory_const_js_1.BuildDirConst.MAPPING);
  }
  getPackageHapProductPath() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.PACKAGE_HAP, this._targetName);
  }
  getFastAppStorageDirectory() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.FAST_APP_DIR, this._targetName);
  }
  getDependencyResourceStr(t) {
    return inject_util_js_1.InjectUtil.isLocalTest() ? path_1.resolve(t, common_const_js_1.UnitTestConst.TEST_BUILD_ROOT, this._productName, build_directory_const_js_1.BuildDirConst.INTERMEDIATES, build_directory_const_js_1.BuildDirConst.INTERMEDIATES_RES, this._targetName, build_directory_const_js_1.BuildDirConst.RESOURCE_STR) : path_1.resolve(t, build_directory_const_js_1.BuildDirConst.BUILD_ROOT, this._productName, build_directory_const_js_1.BuildDirConst.INTERMEDIATES, build_directory_const_js_1.BuildDirConst.INTERMEDIATES_RES, this._targetName, build_directory_const_js_1.BuildDirConst.RESOURCE_STR);
  }
  getSymbolsDir(t) {
    return t ? path_1.resolve(t, build_directory_const_js_1.BuildDirConst.BUILD_ROOT, this._productName, build_directory_const_js_1.BuildDirConst.BUILD_OUTPUTS, this._targetName, build_directory_const_js_1.BuildDirConst.OUTPUT_SYMBOL) : "";
  }
  getResourceStr() {
    return path_1.resolve(this.getIntermediatesRes(), build_directory_const_js_1.BuildDirConst.RESOURCE_STR);
  }
  getLoaderJsonPath() {
    return path_1.resolve(this.getIntermediatesLoaderPath(), build_directory_const_js_1.BuildArtifactConst.LOADER_JSON);
  }
  getSourceHarDuplicateFilesInfoPath() {
    return path_1.resolve(this.getIntermediatesHarDuplicatePath(), build_directory_const_js_1.BuildArtifactConst.SOURCE_HAR_DUPLICATE_FILES_INFO);
  }
  getByteCodeHarDuplicateFilesInfoPath() {
    return path_1.resolve(this.getIntermediatesHarDuplicatePath(), build_directory_const_js_1.BuildArtifactConst.BYTE_CODE_HAR_DUPLICATE_FILES_INFO);
  }
  getHarDuplicateInfoPath() {
    return path_1.resolve(this.getIntermediatesHarDuplicatePath(), build_directory_const_js_1.BuildArtifactConst.HAR_DUPLICATE_INFO);
  }
  getPkgContextInfoDuplicateInfoPath() {
    return path_1.resolve(this.getIntermediatesHarDuplicatePath(), build_directory_const_js_1.BuildArtifactConst.PKG_CONTEXT_INFO_DUPLICATE_INFO);
  }
  getResourcesDuplicateInfoPath() {
    return path_1.resolve(this.getIntermediatesHarDuplicatePath(), build_directory_const_js_1.BuildArtifactConst.RESOURCES_DUPLICATE_INFO);
  }
  getRouterMapDuplicateInfoPath() {
    return path_1.resolve(this.getIntermediatesHarDuplicatePath(), build_directory_const_js_1.BuildArtifactConst.ROUTER_MAP_DUPLICATE_INFO);
  }
  getLoaderRouterMapDuplicateInfoPath() {
    return path_1.resolve(this.getIntermediatesHarDuplicatePath(), build_directory_const_js_1.BuildArtifactConst.LOADER_ROUTER_MAP_DUPLICATE_INFO);
  }
  getStartupDuplicateInfoPath() {
    return path_1.resolve(this.getIntermediatesHarDuplicatePath(), build_directory_const_js_1.BuildArtifactConst.START_UP_DUPLICATE_INFO);
  }
  getAbilityDuplicateInfoPath() {
    return path_1.resolve(this.getIntermediatesHarDuplicatePath(), build_directory_const_js_1.BuildArtifactConst.ABILITY_DUPLICATE_INFO);
  }
  getSoDuplicateInfoPath() {
    return path_1.resolve(this.getIntermediatesHarDuplicatePath(), build_directory_const_js_1.BuildArtifactConst.SO_DUPLICATE_INFO);
  }
  getWidgetLoaderJsonPath() {
    return path_1.resolve(this.getIntermediatesLoaderPath(), build_directory_const_js_1.BuildArtifactConst.WIDGET_LOADER_JSON);
  }
  getObfCachePath() {
    return path_1.resolve(this.getModuleBuildCachePath(), `${this._targetName}@${this._curModule.isHarModule() ? "HarCompileArkTS" : "CompileArkTS"}`, "esmodule", "release", "obfuscation");
  }
  getObfNameCachePath() {
    return path_1.resolve(this.getObfCachePath(), build_directory_const_js_1.BuildArtifactConst.NAME_CACHE);
  }
  getByteCodeObfConfigPath() {
    return path_1.resolve(this.getObfCachePath(), build_directory_const_js_1.BuildArtifactConst.BYTE_CODE_OBF_CONFIG);
  }
  getPreloadSoFilePath() {
    return path_1.resolve(this.getModuleBuildIntermediates(), build_directory_const_js_1.BuildDirConst.PRELOAD, this._targetName, build_directory_const_js_1.BuildArtifactConst.PRELOAD_SO_FILE_NAME);
  }
}
