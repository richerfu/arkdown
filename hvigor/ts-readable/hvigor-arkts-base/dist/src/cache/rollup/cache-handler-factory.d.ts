/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/cache/rollup/cache-handler-factory.d.ts
 */
import { MemoryCacheHandler } from './handle/memory-cache-handler';
import { CacheStoreManager } from '../../util/cache-store-type';
/**
 * 获取缓存处理器的链，本工厂负责new出来并拼装好next的顺序
 */
export declare class CacheHandlerFactory {
    static getHandlerChain(cacheDir: string, cacheStoreManager: CacheStoreManager | undefined): MemoryCacheHandler;
}
