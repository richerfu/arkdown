/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/superior/hos-sdk-component.d.ts
 */
import { Component } from '@ohos/sdkmanager-common';
/**
 * hos sdk公共实例
 *
 * @since 2021-4-18
 */
export declare class HosSdkComponent {
    private readonly _baseDir;
    private readonly _component;
    constructor(component: Component, type: string);
    getBaseDir(): string;
    getComponent(): Component;
}
