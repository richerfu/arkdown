/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/index.d.ts
 */
export { runHvigorCompile } from './src/hvigor-compile';
export { ArktsBaseLogger, LogEvent } from './src/log/arkts-base-log';
export { terserPlugin } from './src/plugins/terser-plugin';
export { WatchEvent } from './src/util/build-event';
export { DEFAULT_COMPILE_CACHE_FILE_NAME, MSGPACK_COMPILE_CACHE_DIR_NAME, NON_ENTRY_IMPORTEE_CACHE_FILE_NAME, COMPILED_IMPORTEE_CACHE_FILE_NAME, DIRECT_IMPORTEE_CACHE_FILE_NAME, } from './src/util/constants';
export { writeWatchCache } from './src/build-steps/watch';
export { BuildResult } from './src/util/build-result';
export * from 'rollup';
export { CacheStoreManager, CacheStore } from './src/util/cache-store-type';
export { findSuitableFilePath } from './src/util/utils';
export { HvigorRollupLogger } from './src/log/hvigor-rollup-logger';
export { projectConfigPath } from './src/util/project-config-path';
export { getHashByFilePath, setHashValueByFilePath } from 'rollup';
export { rollupCacheService } from './src/cache/rollup/rollup-cache-service';
export { addCompilerLogMsg, CompilerLogMsg } from './src/log/ohos-logger';
