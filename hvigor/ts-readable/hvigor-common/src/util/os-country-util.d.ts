/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/util/os-country-util.d.ts
 */
export declare enum countryEnum {
    CN = "cn",
    EN = "en"
}
/**
 * 获取系统语言
 */
export declare function getOsLanguage(): countryEnum;
