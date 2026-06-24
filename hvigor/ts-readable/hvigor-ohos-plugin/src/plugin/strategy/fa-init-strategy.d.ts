/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/strategy/fa-init-strategy.d.ts
 */
import { InitializationStrategy } from './initialization-strategy';
import { Module, Project } from '@ohos/hvigor';
import { LegacyHapPlugin } from '../legacy/legacy-hap-plugin';
import { LegacyHarPlugin } from '../legacy/legacy-har-plugin';
import { LegacyAppPlugin } from '../legacy/legacy-app-plugin';
import { LegacyProjectModelImpl } from '../../model/project/legacy-project-model-impl';
export declare class FaInitializationStrategy extends InitializationStrategy {
    private _log;
    constructor();
    createAppPlugin(project: Project): LegacyAppPlugin;
    createHapPlugin(module: Module): LegacyHapPlugin;
    createHarPlugin(module: Module): LegacyHarPlugin;
    /**
     * 创建工程模型
     *
     * @param module - 模块对象，包含项目信息
     * @returns LegacyProjectModelImpl - 返回项目的FA模型实现
     */
    createProjectModel(module: Module): LegacyProjectModelImpl;
    /**
     * 初始化插件参数
     *
     * @param plugin - 抽象模块插件对象，用于初始化
     * @param module - 模块对象，包含项目信息
     * @param isHapModule - 布尔值，指示是否为HAP模块
     * @returns AbstractModulePlugin - 返回初始化后的插件
     */
    private initPluginParam;
    /**
     * 设置模块插件的构建配置、任务等信息
     *
     * @param plugin - 抽象模块插件对象，用于初始化
     * @param module - 模块对象，包含项目信息
     */
    private setUpModulePlugin;
}
