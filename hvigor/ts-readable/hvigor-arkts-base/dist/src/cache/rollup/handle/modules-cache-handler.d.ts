/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/cache/rollup/handle/modules-cache-handler.d.ts
 */
import { ModuleJSON, RollupCache } from 'rollup';
import { CompileEvent } from '../../../options/hvigor-options';
import { CacheType, RollupBuildPartial } from './abstract-cache-handler';
import { AbstractFileCacheHandler } from './abstract-file-cache-handler';
/**
 * modules缓存的处理器，负责读写rollup的modules缓存
 */
export declare class ModulesCacheHandler extends AbstractFileCacheHandler<ModuleJSON> {
    handlerName: string;
    handlerReadResult(optionCache: RollupCache, readResult: ModuleJSON): void;
    /**
     * 根据构建结果 决定是新文件覆盖旧文件还是移除新文件
     * @param bundle
     * @param compileEnv
     * @param isBuildSuccess
     * @param cache
     */
    onBuildEnd(bundle: RollupBuildPartial, compileEnv: string | undefined, isBuildSuccess: boolean, cache: CacheType): Promise<void>;
    deleteModuleCache(moduleId: string, compileEnv?: string): void;
    writeFileCache(cache: CacheType, allModules: Set<string>, writePrm: Promise<void>[], usePathPlaceholder: boolean, packCacheEvent?: CompileEvent, changedModules?: Record<string, any>): void;
}
