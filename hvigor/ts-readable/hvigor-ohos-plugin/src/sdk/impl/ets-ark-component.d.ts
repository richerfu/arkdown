/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/impl/ets-ark-component.d.ts
 */
import { ArkComponent } from '../superior/ark-component';
/**
 * js ark实例
 *
 * @since 2021-12-16
 */
export declare class EtsArkComponent implements ArkComponent {
    private _log;
    private readonly _ets2abcToolPath;
    private ets2abcVersion;
    constructor(etsLoaderPath: string);
    getArkVersion(compatibleSdkVersion?: number, compatibleSdkVersionStage?: string): string;
}
