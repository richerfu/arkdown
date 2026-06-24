/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/options/configure/arkui-x-config-opt.d.ts
 */
export declare namespace ArkUIXConfigJson {
    interface ConfigObj {
        crossplatform: boolean;
        modules: string[];
        buildOption?: {
            resConfigs?: string[];
            ignoreCrossPlatform?: boolean;
        };
    }
}
