/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/data/code-snippet.d.ts
 */
export type CodeSnippet = {
    script: string;
    offset: Offset;
    filePath: string;
    log: Offset[];
};
type Offset = {
    line: string;
    column: string;
};
export {};
