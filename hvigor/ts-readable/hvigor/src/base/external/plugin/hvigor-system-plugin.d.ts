/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/plugin/hvigor-system-plugin.d.ts
 */
export declare abstract class HvigorSystemPlugin {
    protected pluginVersion: string;
    protected readonly pluginId: string;
    protected constructor(pluginId: string);
    getPluginId(): string;
    abstract getContext(): any;
}
