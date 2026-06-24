/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/cache/rollup/handle/abstract-file-cache-handler.d.ts
 */
import { RollupCache } from 'rollup';
import { CompileEvent, HvigorOptions } from '../../../options/hvigor-options';
import { CacheStoreManager } from '../../../util/cache-store-type';
import { AbstractCacheHandler, CacheParam, CacheType } from './abstract-cache-handler';
export declare abstract class AbstractFileCacheHandler<T> extends AbstractCacheHandler {
    cacheStoreManager: CacheStoreManager | undefined;
    constructor(cacheDir: string, cacheStoreManager: CacheStoreManager | undefined);
    private processCacheDir;
    readCache(options: HvigorOptions): void;
    writeCache(cacheParam: CacheParam): void;
    abstract handlerReadResult(optionCache: RollupCache, readResult: T, cacheMsgpackAbsolutePath: string): void;
    protected abstract writeFileCache(cache: CacheType, allModules: Set<string>, writePrm: Promise<void>[], usePathPlaceholder: boolean, packCacheEvent?: CompileEvent, changedModules?: Record<string, any>): void;
    protected getHashName(moduleId: string): string;
}
