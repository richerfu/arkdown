/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/magic-string-replacement/code-fragment.d.ts
 */
export declare class CodeFragment {
    readonly start: number;
    readonly end: number;
    prefix: string;
    readonly sourceCode: string;
    suffix: string;
    content: string;
    isOverwritten: boolean;
    prev?: CodeFragment;
    next?: CodeFragment;
    constructor(params: {
        start: number;
        end: number;
        fragmentSourceCode: string;
    });
    toString(): string;
    isInRange(index: number): boolean;
    overwrite(content: string): void;
}
