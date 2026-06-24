/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor/pack-tool-adaptor.d.ts
 */
import { ThirdPartyErrorAdaptor } from './third-party-error-adaptor';
/**
 * 打包工具的适配器
 */
export declare class PackToolAdaptor extends ThirdPartyErrorAdaptor {
    constructor(errorMessage: string);
    analyze(errorMessage: string): void;
    getErrorCodeJsonPath(): string[];
}
