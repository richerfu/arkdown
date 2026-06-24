/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/plugin/default-plugin-container.d.ts
 */
import { HvigorSystemPlugin } from './hvigor-system-plugin';
import { PluginContainer } from './plugin-container-interface';
/**
 * 用于保存当前module在hvigorFile.js中exports出来的plugin
 *
 * @since 2022/5/5
 */
export declare class DefaultPluginContainer implements PluginContainer {
    private readonly _pluginMap;
    constructor();
    registryPlugin(plugin: HvigorSystemPlugin): HvigorSystemPlugin;
    getPluginById(pluginId: string): HvigorSystemPlugin | undefined;
}
