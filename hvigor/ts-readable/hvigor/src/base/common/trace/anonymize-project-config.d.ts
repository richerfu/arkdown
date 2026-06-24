/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/trace/anonymize-project-config.d.ts
 */
export declare class AnonymizeProjectConfig {
    private readonly PROJECT_CONFIG_KEYS;
    private projectConfigRaw;
    anonymizeProjectConfig(projectConfig?: {
        [k: string]: any;
    }): {
        [k: string]: any;
    };
    /**
     * ArkTS工程，获取脱敏后的projectConfig对象
     * @private
     */
    private getAnonymizedProjectConfigArkTS;
}
