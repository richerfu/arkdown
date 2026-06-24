/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/sdk-java-component.d.ts
 */
import { Component } from '@ohos/sdkmanager-common';
import { HosSdkComponent } from '../superior/hos-sdk-component';
/**
 * sdk java实例
 */
export declare class SdkJavaComponent extends HosSdkComponent {
    private static readonly BUILD_TOOLS;
    constructor(component: Component);
    getRestoolPath(): string;
    getReleaseType(): string;
}
