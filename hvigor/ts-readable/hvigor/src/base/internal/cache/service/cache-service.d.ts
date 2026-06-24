/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/service/cache-service.d.ts
 */
export interface CacheService<T> {
    initialize(): void;
    get(key: string): T | undefined;
    set(key: string, entryContent: T): void;
    remove(key: string): void;
    close(): void;
}
