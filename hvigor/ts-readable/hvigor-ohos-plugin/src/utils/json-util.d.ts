/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/json-util.d.ts
 */
export declare class JsonUtil {
    static getJson5Obj(json5path: string, encodingStr?: string): any;
}
/**
 * 这个函数会抛异常，需要使用方try-catch
 *
 * @param json5path
 * @param encodingStr
 */
export declare function getJson5ObjThrowError(json5path: string, encodingStr?: string): any;
/**
 * 合并两个String json的数据
 * @param data1
 * @param data2
 */
export declare function mergeStringJson(data1: any, data2: any): {
    string: StringRes[];
};
export interface StringRes {
    name: string;
    value: string;
}
