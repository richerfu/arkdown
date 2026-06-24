/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/json-schema-util.d.ts
 */
export type ErrorInfo = {
    instancePath: string;
    keyword: string;
    params: Record<string, any>;
    message?: string;
    location?: string;
    help?: string;
    solution?: string;
    reason?: string;
};
/**
 * @param data 被校验的对象
 * @param schemaKey schema的缓存key
 * @param schema schema对象，可以只传入schema.json的一部分
 */
export declare function validateDataBySchema(data: any, schemaKey: string, schema: Record<string, any>): {
    isValid: boolean;
    errMsg: string;
};
