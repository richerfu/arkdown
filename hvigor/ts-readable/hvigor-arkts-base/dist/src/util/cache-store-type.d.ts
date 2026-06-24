/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/util/cache-store-type.d.ts
 */
export interface CacheStore {
    getCache(key: string): any;
    setCache(key: string, value: any, pinned?: boolean): CacheStore;
    get(key: string): any;
    set(key: string, value: any, pinned?: boolean): CacheStore;
    hasCache(key: string): boolean;
    clear(): void;
    delete(key: string): boolean;
    size(): number;
    keys(): IterableIterator<string>;
}
export interface CacheStoreManager {
    mount(key: string, pinned?: boolean): CacheStore;
    unmount(key: string): boolean;
    clear(): void;
    size(): number;
    keys(): IterableIterator<string>;
}
