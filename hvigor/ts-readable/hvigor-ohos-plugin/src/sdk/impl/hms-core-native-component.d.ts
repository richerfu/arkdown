/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/hms-core-native-component.d.ts
 */
import { Component } from '@ohos/sdkmanager-common';
import { SdkComponent } from '../superior/sdk-component';
export declare class HmsCoreNativeComponent extends SdkComponent {
    private readonly _binxoTool;
    constructor(component: Component);
    getHmsToolchainFile(): string;
    getHmsBiShengToolchainFile(): string;
    getSdkBinxoTool(): string;
}
