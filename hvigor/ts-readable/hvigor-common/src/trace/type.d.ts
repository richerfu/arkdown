/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-common/src/trace/type.d.ts
 */
export interface JSON_VALUE_RECORD {
    path: string;
    traceValue: boolean;
    value: any;
}
export type TelemetryRule = {
    trace: boolean;
    traceValue: boolean;
};
