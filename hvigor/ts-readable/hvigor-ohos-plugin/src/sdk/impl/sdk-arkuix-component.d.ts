/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/sdk-arkuix-component.d.ts
 */
import { Component } from '@ohos/sdkmanager-common';
import { SdkComponent } from '../superior/sdk-component';
/**
 * arkuix 实例
 *
 * @since 2023-05-04
 */
export declare class SdkArkUIXComponent extends SdkComponent {
    readonly ARKUI_X_ROOT = "arkui-x";
    constructor(component: Component);
    getArkUIXRootPath(): string;
}
