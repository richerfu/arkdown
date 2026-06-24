/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/build-steps/rm-error-files-cache.d.ts
 */
import { RollupCache } from 'rollup';
import { HvigorOptions } from '../options/hvigor-options';
import { CacheStoreManager } from '../util/cache-store-type';
export declare function removeCompileFailedFilesFromCache(options: HvigorOptions, cacheDirectory?: string, cacheStoreManager?: CacheStoreManager, compileEnv?: string, cache?: RollupCache | false): Promise<void>;
