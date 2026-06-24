/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/json-reader.d.ts
 */
export declare class JsonReader {
    private static logger;
    static getJsonText(jsonPath: string, encodingStr?: string): string;
    /**
     * 获取json对象
     *
     * @param {string} jsonPath json路径
     * @param {any} encodingStr 编码方式
     */
    static getJsonObj(jsonPath: string, encodingStr?: string): any;
    /**
     * 解析字符串为对象expected
     *
     * @param text 解析字符串
     * @param expected 期望返回对象类型实例, 也作为failsafe返回
     * @param reviver 自定义解析规则
     */
    static parseJsonText<T>(text: string, expected?: T, reviver?: (this: any, key: string, value: any) => any): T;
}
