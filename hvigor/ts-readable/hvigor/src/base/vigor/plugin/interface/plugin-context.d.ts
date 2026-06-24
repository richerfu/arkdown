/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/vigor/plugin/interface/plugin-context.d.ts
 */
import { HvigorLogger } from '../../../log/hvigor-log';
export declare class PluginContext {
    protected logger: HvigorLogger;
    private contextMap;
    setContextFunc(pluginId: string, func: Function): void;
    getContext(pluginId: string): any;
}
