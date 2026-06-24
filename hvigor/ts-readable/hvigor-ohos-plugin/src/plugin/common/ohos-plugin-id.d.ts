/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/common/ohos-plugin-id.d.ts
 */
export declare class OhosPluginId {
    static readonly OHOS_APP_PLUGIN = "com.ohos.app";
    static readonly OHOS_HAP_PLUGIN = "com.ohos.hap";
    static readonly OHOS_HAR_PLUGIN = "com.ohos.har";
    static readonly OHOS_HSP_PLUGIN = "com.ohos.hsp";
}
/**
 * OhosPlugin 对应不同模块使用的插件id
 *
 * @deprecated 此接口类型名称已废弃，不再推荐使用，请使用OhosPluginId替代
 */
export type DefaultPluginId = OhosPluginId;
