/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/report/report.d.ts
 */
export declare class Report<T> {
    private readonly name;
    private readonly value;
    constructor(name: string, report: T);
    /**
     * 获取报告名
     */
    getName(): string;
    /**
     * 获取报告内容
     */
    getValue(): T;
}
