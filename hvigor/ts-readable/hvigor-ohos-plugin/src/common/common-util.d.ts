/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/common/common-util.d.ts
 */
export declare function isSingleFramework(): boolean;
/**
 * 解析$profile形式的配置文件名称
 * ($profile:form_config解析为form_config)
 *
 * @return {string} 返回解析后的名称
 */
export declare function parsingProfileName(srcStr: string): string | undefined;
/**
 * 替换字符串中带${} 的内容
 * @param source
 * @param target 目标值的来源
 */
export declare function replaceStringParam(source: string | undefined, target: Record<string, unknown> | undefined): string | undefined;
