/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/cache/rollup/handle/abstract-cache-handler.d.ts
 */
import { ModuleJSON, RollupBuild, RollupCache, SerializablePluginCache } from 'rollup';
import { CompileEvent, HvigorOptions } from '../../../options/hvigor-options';
/**
 * 责任链模式实现缓存读取写入，compiler.cache下的每个目录都是一份相对独立的缓存，目前包含modules,plugins,other
 *
 * 文件目录结构:
 * compiler.cache(.temp) 未编译成功时有后缀，完成后删除
 *  |-modules
 *  |-plugins(可选)
 *  |-other 目前包含 pkgName2SourceRoots
 */
export declare abstract class AbstractCacheHandler {
    abstract handlerName: string;
    cacheDir: string;
    next: AbstractCacheHandler | undefined;
    abstract readCache(options: HvigorOptions): void;
    abstract writeCache(cacheParam: CacheParam): void;
    onBuildEnd(bundle: RollupBuildPartial, compileEnv: string | undefined, isBuildSuccess: boolean, cache: CacheType): Promise<void>;
    deleteModuleCache(moduleId: string, compileEnv?: string): void;
    constructor(cacheDir: string);
    protected getCacheAbsolutePath(): string;
    setNext(next: AbstractCacheHandler): void;
    protected filterEvent(parentEvent?: CompileEvent, ChildEvent?: CompileEvent): void;
}
export type CacheType = false | RollupCache | {
    pkgName2SourceRoots: Record<string, string[]>;
    modules: ModuleJSON[];
    plugins?: Record<string, SerializablePluginCache>;
} | undefined;
export interface CacheParam {
    cache: CacheType;
    allModules: Set<string>;
    writePrm: Promise<void>[];
    packCacheEvent?: CompileEvent;
    changedModules?: Record<string, any>;
    usePathPlaceholder: boolean;
}
export type RollupBuildPartial = Pick<RollupBuild, 'close' | 'cache'>;
