/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/enum/runtime-type-enum.d.ts
 */
export declare enum RuntimeTypeEnum {
    UNDEFINED = 0,
    OpenHarmony = 1,
    HARMONY_OS = 2
}
export declare namespace RuntimeTypeEnum {
    function valueOf(runtimeOSStr: string | undefined): RuntimeTypeEnum;
}
