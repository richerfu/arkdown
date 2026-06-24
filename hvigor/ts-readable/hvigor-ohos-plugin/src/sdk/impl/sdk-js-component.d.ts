/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/sdk-js-component.d.ts
 */
import { Component } from '@ohos/sdkmanager-common';
import { SdkComponent } from '../superior/sdk-component';
/**
 * sdk js实例
 *
 * @since 2021-12-16
 */
export declare class SdkJsComponent extends SdkComponent {
    private readonly _jsLoaderPath;
    private readonly _hapToBinPath;
    private readonly _sysCapFilePath;
    private arkComponent;
    constructor(component: Component);
    getJsLoaderPath(): string;
    getArkVersion(): string;
    getCompatibleArkVersion(compatibleSdkVersion: number, compatibleSdkVersionStage?: string): string;
    getSysCapFilePath(): string;
    getHapToBinFilePath(): string;
}
