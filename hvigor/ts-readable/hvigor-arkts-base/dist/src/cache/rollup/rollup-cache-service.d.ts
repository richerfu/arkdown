/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/cache/rollup/rollup-cache-service.d.ts
 */
import { RollupCache } from 'rollup';
import { CompileEvent, HvigorOptions } from '../../options/hvigor-options';
import { CacheStoreManager } from '../../util/cache-store-type';
import { RollupBuildPartial } from './handle/abstract-cache-handler';
/**
 * 对外提供关于rollup的缓存的服务
 */
declare class RollupCacheService {
    readCache(options: HvigorOptions, cacheStoreManager?: CacheStoreManager): void;
    writeCache({ enableFileSystemCache, cache, cacheDirectory, pkgName2SourceRoots, cacheKey, cacheStoreManager, useTempFile, parentEvent, changedModules, usePathPlaceholder }: WriteCacheParam): Promise<void>;
    onBuildEnd(cacheDirectory: string, bundle: RollupBuildPartial, compileEnv: string | undefined, isBuildSuccess: boolean, pkgName2SourceRoots?: Record<string, string[]>, cacheStoreManager?: CacheStoreManager): Promise<void>;
    getCacheDir(userConfigCacheDirectory: string | undefined): string;
    private getFinalCache;
    /**
     * 删除所有内存中文件的缓存
     */
    deleteModuleCache(moduleId: string, cachePath?: string, cacheStoreManager?: CacheStoreManager, compileEnv?: string): void;
    deleteCache(cacheDirectory: string | undefined): void;
    /**
     * 当一个模块中通过targetSourceRoots的方式来import另外一个模块后，存在以下场景不能继续使用缓存
     * 1. 模块对应target的sourceRoots进行了变更
     * 2. 优先级较高的sourceRoot进行了调整，且符合import路径规则
     * @param options
     */
    removeUncacheableModules(options: HvigorOptions): void;
}
interface WriteCacheParam {
    enableFileSystemCache: boolean | undefined;
    cache: RollupCache | undefined;
    cacheDirectory: string | undefined;
    pkgName2SourceRoots?: Record<string, string[]>;
    cacheKey?: string;
    cacheStoreManager?: CacheStoreManager;
    useTempFile?: boolean;
    parentEvent?: CompileEvent;
    changedModules?: Set<string>;
    usePathPlaceholder: boolean;
}
export declare const rollupCacheService: RollupCacheService;
export {};
