/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/entry/cache-entry.d.ts
 */
export declare class CacheEntry<T> {
    private readonly _key;
    private readonly _content;
    constructor(key: string, content: T);
    getKey(): string;
    getContent(): T;
}
