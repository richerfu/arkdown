/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/enum/code-type-enum.d.ts
 */
export declare enum CodeType {
    JS = "js",
    ETS = "ets",
    CPP = "cpp"
}
/**
 * SDK 分类
 *
 * @since 2023/05/12
 */
export declare enum SdkTypeEnum {
    HOS = "HOS",
    OHOS = "OHOS",
    ARKUI_X = "ARKUI-X"
}
/**
 * 为枚举类附加特定方法
 *
 * @since 2022/1/26
 */
export declare namespace CodeType {
    function getSDKComponentName(codeType: CodeType): string;
}
