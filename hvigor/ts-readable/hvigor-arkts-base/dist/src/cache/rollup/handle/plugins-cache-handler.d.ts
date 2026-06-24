/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/cache/rollup/handle/plugins-cache-handler.d.ts
 */
import { CacheType } from './abstract-cache-handler';
import { CompileEvent } from '../../../options/hvigor-options';
import { AbstractFileCacheHandler } from './abstract-file-cache-handler';
import { RollupCache, SerializablePluginCache } from 'rollup';
export declare class PluginsCacheHandler extends AbstractFileCacheHandler<Record<string, SerializablePluginCache>> {
    handlerName: string;
    handlerReadResult(optionCache: RollupCache, readResult: Record<string, SerializablePluginCache>): void;
    writeFileCache(cache: CacheType, allModules: Set<string>, writePrm: Promise<void>[], usePathPlaceholder: boolean, packCacheEvent?: CompileEvent, changedModules?: Record<string, any>): void;
}
