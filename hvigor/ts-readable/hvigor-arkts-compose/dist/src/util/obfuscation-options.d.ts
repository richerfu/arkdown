/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/obfuscation-options.d.ts
 */
export interface RuleOptions {
    enable?: boolean;
    files?: string | string[];
    rules?: string[];
}
export interface Obfuscation {
    ruleOptions?: RuleOptions;
    consumerFiles?: string | string[];
    consumerRules?: string[];
    libDir: string;
}
export interface ObfuscationConfig {
    selfConfig?: Obfuscation;
    dependencies: ObfuscationDependencies;
}
/**
 * 字段名由OS确定，传递给OS的参数
 *
 * libraries: 本地har的混淆文件
 * hspLibraries: 本地hsp的混淆文件
 * hars: 远程har的混淆文件obfuscation.txt
 * hsps: 远程hsp的混淆文件obfuscation.txt
 */
export interface ObfuscationDependencies {
    libraries: Obfuscation[];
    hspLibraries: Obfuscation[];
    hars: string[];
    hsps: string[];
}
export interface ObfuscationOptions extends ObfuscationConfig {
    sdkApis: string[];
    obfuscationCacheDir: string;
    exportRulePath: string;
}
