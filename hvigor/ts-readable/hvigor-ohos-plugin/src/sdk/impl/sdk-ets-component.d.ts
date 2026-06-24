/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/sdk-ets-component.d.ts
 */
import { Component } from '@ohos/sdkmanager-common';
import { SdkComponent } from '../superior/sdk-component';
/**
 * sdk ets实例
 *
 * @since 2021-12-16
 */
export declare class SdkEtsComponent extends SdkComponent {
    private readonly _etsLoaderPath;
    private readonly _arkComponent;
    private readonly _sysCapFilePath;
    constructor(component: Component);
    getEtsLoaderPath(): string;
    getArkVersion(): string;
    getCompatibleArkVersion(compatibleSdkVersion: number, compatibleSdkVersionStage?: string): string;
    getSysCapFilePath(): string;
}
