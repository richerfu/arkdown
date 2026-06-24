/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/sdk-native-component.d.ts
 */
import { Component } from '@ohos/sdkmanager-common';
import { SdkComponent } from '../superior/sdk-component';
/**
 * ohos sdk native实例类
 *
 * @since 2021/1/21
 */
export declare class SdkNativeComponent extends SdkComponent {
    private readonly _ninjaTool;
    private readonly _cmakeTool;
    private readonly _nativeToolchain;
    private readonly _llvmStrip;
    private readonly _llvmReadElf;
    constructor(component: Component);
    getCmakeTool(): string;
    getNinjaTool(): string;
    getNativeToolchain(): string;
    getLlvmStrip(): string;
    getLlvmReadElf(): string;
}
