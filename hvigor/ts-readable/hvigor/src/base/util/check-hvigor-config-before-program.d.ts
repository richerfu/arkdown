/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/util/check-hvigor-config-before-program.d.ts
 */
export declare function checkHvigorConfigBeforeProgram(cliArgv: any, logger: any): {
    result: boolean;
    stack?: undefined;
    message?: undefined;
} | {
    result: boolean;
    stack: any;
    message: string;
};
