/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/cache/rollup/handle/other-cache-handler.d.ts
 */
import { AbstractFileCacheHandler } from './abstract-file-cache-handler';
import { RollupCache } from 'rollup';
import { CacheType } from './abstract-cache-handler';
import { CompileEvent } from '../../../options/hvigor-options';
export declare class OtherCacheHandler extends AbstractFileCacheHandler<object> {
    handlerName: string;
    handlerReadResult(optionCache: RollupCache, readResult: object, cacheMsgpackAbsolutePath: string): void;
    writeFileCache(cache: CacheType, allModules: Set<string>, writePrm: Promise<void>[], usePathPlaceholder: boolean, packCacheEvent?: CompileEvent, changedModules?: Record<string, any>): void;
}
