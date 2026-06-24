/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/builder/base-command-builder.d.ts
 */
export declare class BaseCommandBuilder {
    protected commandList: string[];
    constructor(commandList: string[]);
    build(): string[];
    addAllParams(params: string[]): string[];
}
