/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/delete-plugin.d.ts
 */
interface DelOptions {
    hook?: string;
    runOnce?: boolean;
    targets?: string[];
}
export default function delPlugin(options?: DelOptions): {
    [x: string]: string | (() => Promise<void>);
    name: string;
};
export {};
