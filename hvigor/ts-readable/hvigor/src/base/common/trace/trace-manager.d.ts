/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/trace/trace-manager.d.ts
 */
export declare class TraceManager {
    private readonly data;
    private callBackList;
    constructor();
    /**
     * 记录数据, 以项目为单位
     * @param {string} key 项目名
     * @param data 打点数据
     * @param {Function} callback 回调，会在落盘后执行
     */
    trace(key: string, data: any, callback?: Function): void;
    /**
     * 将打点数据落盘：.hvigor/outputs/logs/details/details.json
     */
    flush(): void;
    /**
     * 打点数据的key值不能有'.'
     * @param {string} key
     * @returns {string}
     */
    static transformKey(key: string): string;
    /**
     * 对字符串匿名化处理
     * @param {string} str
     * @returns {string}
     */
    static anonymize(str: string): string;
    static trace(key: string, data: any, callback?: Function): void;
}
export declare const traceManager: TraceManager;
