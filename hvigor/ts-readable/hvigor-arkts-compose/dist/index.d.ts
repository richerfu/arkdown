/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/index.d.ts
 */
export { runArkPack, compileCodeSnippet, watch } from './src/arkts-pack';
export type { ProjectConfig, ArkCompileProjectModelType, ModuleInfoType } from './src/project-config/project-config';
export type { Path2ohmurl, PkgBriefInfo, DependencyInfoType, DuplicatedHarDependency, OhExportsDependencyInfo, BuildOption } from './src/project-config/build-option/build-option';
export type { MockConfigModuleInfo } from './src/project-config/mock-info';
export { writeWatchCache, LogEvent, MSGPACK_COMPILE_CACHE_DIR_NAME, findSuitableFilePath, NON_ENTRY_IMPORTEE_CACHE_FILE_NAME, COMPILED_IMPORTEE_CACHE_FILE_NAME, DIRECT_IMPORTEE_CACHE_FILE_NAME, } from '@ohos/hvigor-arkts-base';
export * from './src/hvigor-plugins/types';
export * from './src/util/obfuscation-options';
export { generateObfuscationFile, runEtsStandaloneChecker } from './src/util/load-sdk-functions';
export { CompileEvent } from './src/analyzer/compile-event';
export { generateReport, getPkgName2SourceRootsMap } from './src/util/utils';
export { GenerateOhosTestTemplateParam, generateOhosTestTemplateFile as generateOhosTestTemplate, generateTemplateFile as generateUnitTestTemplate, executeCoverageTest as generateUnitTestResult, executeCoverageOhosTest as generateOhosTestResult, } from '@ohos/coverage/lib/src/commandLine/index';
export { findNodeEntryFiles } from './src/plugins/module-meta-info-plugin';
export { CacheStoreManager, CacheStore } from '@ohos/hvigor-arkts-base';
export { SUPPORT_EXTENSIONS, TRANSFORM_ABC_WORKER_REQUEST } from './src/util/constants';
export { ExtendedErrorInfo } from './src/data/compose-error';
