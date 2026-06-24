/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/strategy/stage-init-strategy.d.ts
 */
import { InitializationStrategy } from './initialization-strategy';
import { Module, Project } from '@ohos/hvigor';
import { ProjectModelImpl } from '../../model/project/project-model-impl';
import { AppPlugin } from '../app-plugin';
import { HapPlugin } from '../hap-plugin';
import { HspPlugin } from '../hsp-plugin';
import { HarPlugin } from '../har-plugin';
import { HapPluginBuilder } from '../builder/hap-plugin-builder';
import { HspPluginBuilder } from '../builder/hsp-plugin-builder';
import { HarPluginBuilder } from '../builder/har-plugin-builder';
export declare class StageInitializationStrategy extends InitializationStrategy {
    private _log;
    private cangjieObj;
    constructor();
    createAppPlugin(project: Project): AppPlugin;
    createHapPlugin(module: Module, pluginBuilder: HapPluginBuilder): HapPlugin;
    createHspPlugin(module: Module, pluginBuilder: HspPluginBuilder): HspPlugin;
    createHarPlugin(module: Module, pluginBuiler: HarPluginBuilder): HarPlugin;
    /**
     * 创建工程模型
     *
     * @param module - 模块对象
     * @returns 返回项目模型实例
     */
    createProjectModel(module: Module): ProjectModelImpl;
    /**
     * 初始化插件模型等参数
     *
     * @param plugin - 抽象模块插件对象
     * @param module - 模块对象
     * @param isHapModule - 是否为HAP模块的标志
     * @returns 返回初始化后的抽象模块插件
     */
    private initPluginParam;
    /**
     * 设置模块插件的构建配置、任务等信息
     *
     * @param plugin - 抽象模块插件对象
     * @param module - 模块对象
     */
    private setUpModulePlugin;
}
