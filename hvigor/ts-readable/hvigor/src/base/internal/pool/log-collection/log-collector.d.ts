/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/log-collection/log-collector.d.ts
 */
export declare class LogCollector<T> {
    private readonly logSet;
    constructor();
    /**
     * 添加日志
     *
     * @param log
     */
    add(log: T): void;
    /**
     * 返回所有日志
     *
     * @returns {Set<T>} 日志集合
     */
    dump(): Set<T>;
    /**
     * 清空日志
     */
    clear(): void;
}
