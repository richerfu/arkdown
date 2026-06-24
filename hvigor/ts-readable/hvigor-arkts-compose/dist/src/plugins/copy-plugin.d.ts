/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/copy-plugin.d.ts
 */
interface Ignore {
    ignored: (p: {
        name: string;
    }) => boolean;
}
interface Target {
    src: string;
    dest: string;
    rename?: string | Function;
    ignore?: Ignore;
    onlyFiles?: boolean;
}
interface CopyOptions {
    copyOnce?: boolean;
    hook?: string;
    targets?: Target[];
}
export default function copyPlugin(copyOptions?: CopyOptions): {
    [x: string]: string | (() => Promise<void>);
    name: string;
};
export {};
