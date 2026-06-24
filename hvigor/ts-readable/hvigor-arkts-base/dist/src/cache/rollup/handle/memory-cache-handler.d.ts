/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/cache/rollup/handle/memory-cache-handler.d.ts
 */
import { HvigorOptions } from '../../../options/hvigor-options';
import { CacheStoreManager } from '../../../util/cache-store-type';
import { AbstractCacheHandler, CacheParam, CacheType, RollupBuildPartial } from './abstract-cache-handler';
/**
 * 内存缓存处理器
 */
export declare class MemoryCacheHandler extends AbstractCacheHandler {
    handlerName: string;
    cacheStoreManager: CacheStoreManager | undefined;
    constructor(cacheDir: string, cacheStoreManager: CacheStoreManager | undefined);
    /**
     * 如果内存缓存中存在 触发条件options.cache = cache 继续调用链上其他的处理器
     * 如果内存缓存中不存在，则继续调用下一个处理器
     * @param options
     */
    readCache(options: HvigorOptions): void;
    writeCache(CacheParam: CacheParam): void;
    onBuildEnd(bundle: RollupBuildPartial, compileEnv: string | undefined, isBuildSuccess: boolean, cache: CacheType): Promise<void>;
    deleteModuleCache(moduleId: string, compileEnv?: string): void;
}
