/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/src/error/adaptor/arkTs-error-adaptor.d.ts
 */
import { MatchFieldType } from '../type';
import { ThirdPartyErrorAdaptor } from './third-party-error-adaptor';
export declare class ArkTsErrorAdaptor extends ThirdPartyErrorAdaptor {
    constructor(value: any, field?: MatchFieldType);
    getErrorCodeJsonPath(): string[];
}
