/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/models/build-result.d.ts
 */
export declare class BuildResult {
    private error;
    private report;
    constructor(error: Error | null, report?: any);
    getError(): Error | null;
    private _getError;
    getReportJson(): any;
    private _getReportJson;
}
