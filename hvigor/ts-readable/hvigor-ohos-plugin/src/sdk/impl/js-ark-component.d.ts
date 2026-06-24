/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/js-ark-component.d.ts
 */
import { ArkComponent } from '../superior/ark-component';
/**
 * js ark实例
 */
export declare class JsArkComponent implements ArkComponent {
    private _log;
    private readonly ts2abcToolPath;
    private ts2abcVersion;
    constructor(jsLoaderPath: string);
    getArkVersion(compatibleSdkVersion?: number, compatibleSdkVersionStage?: string): string;
}
