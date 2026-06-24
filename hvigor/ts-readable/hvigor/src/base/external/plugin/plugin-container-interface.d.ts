/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/plugin/plugin-container-interface.d.ts
 */
import { HvigorSystemPlugin } from './hvigor-system-plugin';
/**
 * 对于每一个Hvigor 的Node节点，都可以注册多个自定义的Plugin插件
 * 该接口定义管理插件的相关接口
 *
 * @since 2022/5/6
 */
export interface PluginContainer {
    /**
     * 注册扩展的HvigorPlugin到对应的Hvigor module中
     *
     * @param {HvigorSystemPlugin} plugin
     * @return {HvigorSystemPlugin}
     */
    registryPlugin(plugin: HvigorSystemPlugin): HvigorSystemPlugin;
    /**
     * 通过Plugin Id来获取当前module中存在的plugin
     *
     * @param {string} pluginId
     * @return {HvigorSystemPlugin|undefined}
     */
    getPluginById(pluginId: string): HvigorSystemPlugin | undefined;
}
