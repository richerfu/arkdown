/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/service/map-cache-service.d.ts
 */
export declare abstract class MapCacheService<T> {
    protected cacheEntryMap: Map<string, T>;
    protected constructor();
    /**
     * 初始化函数,默认只需要一个空的map,子类需要根据不同场景进行重载
     */
    initialize(): void;
    close(): void;
    get(key: string): T | undefined;
    remove(key: string): void;
    size(): number;
    /**
     * Set方法需要根据具体场景进行实例化
     *
     * @param {T} entry
     */
    abstract add(entry: T): void;
}
