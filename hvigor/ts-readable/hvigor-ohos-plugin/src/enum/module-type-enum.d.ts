/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/enum/module-type-enum.d.ts
 */
export declare enum ModuleType {
    Entry = "entry",
    Feature = "feature",
    Har = "har",
    Shared = "shared"
}
/**
 * 为枚举类型ModuleType附加方法
 *
 * @since 2022/2/25
 */
export declare namespace ModuleType {
    function valueOf(moduleTypeStr: string): ModuleType;
}
