/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/sdk-toolchains-component.d.ts
 */
import { Component } from '@ohos/sdkmanager-common';
import { SdkComponent } from '../superior/sdk-component';
/**
 * sdk toolchains实例
 */
export declare class SdkToolchainsComponent extends SdkComponent {
    static readonly MODULE_CHECK = "modulecheck";
    static readonly CONFIG_CHECK = "configcheck";
    static readonly SYS_CAP_CHECK = "syscapcheck";
    static readonly PAC_CHECK = "paccheck";
    static readonly RES_CHECK = "rescheck";
    static readonly LIB = "lib";
    static readonly LIBIMAGE_TRANSCODER_SHARED_DLL = "libimage_transcoder_shared.dll";
    static readonly LIBIMAGE_TRANSCODER_SHARED_DYLIB = "libimage_transcoder_shared.dylib";
    static readonly LIBIMAGE_TRANSCODER_SHARED_SO = "libimage_transcoder_shared.so";
    static readonly RICH_SCHEMA = "configSchema_rich.json";
    static readonly LITE_SCHEMA = "configSchema_lite.json";
    constructor(component: Component);
    getRestoolPath(): string;
    getSysCapToolPath(): string;
    getVerifySignConfigToolPath(): string;
    getRichSchema(): string;
    getLiteSchema(): string;
    getPackageToolPath(): string;
    getUnPackageToolPath(): string;
    getSignDir(): string;
    getModuleSchema(): string;
    getAppSchema(): string;
    getAppConfigurationSchema(): string;
    getInsightIntentSchema(): string;
    getArkDataSchema(): string;
    getUtdSchema(): string;
    getFormSchema(): string;
    getPageSchema(): string;
    getRouterMapSchema(): string;
    getShareConfigSchema(): string;
    getShortcutsJsonSchema(): string;
    getAppStartupSchema(): string;
    getHspStartupSchema(): string;
    getHarStartupSchema(): string;
    getStartWindowSchema(): string;
    getSysCapSchema(): string;
    getPacSchema(): string;
    getResIdDefinedSchema(): string;
    getSystemThemeSchema(): string;
}
export declare class HosSdkToolchainsComponent extends SdkToolchainsComponent {
    private component;
    constructor(component: Component);
    getModuleSchema(): string;
    getAppSchema(): string;
    getRichSchema(): string;
    getLiteSchema(): string;
    getHapSignToolV2(): string;
    getHosModuleSchema(): string;
    getReleaseType(): string;
    getHosSignTool(): string;
    getHosToolchainsLibDir(): string;
    getLibimageTranscoderShared(): string;
}
