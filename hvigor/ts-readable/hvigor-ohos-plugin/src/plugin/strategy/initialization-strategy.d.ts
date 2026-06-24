/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/strategy/initialization-strategy.d.ts
 */
import { Module, Project } from '@ohos/hvigor';
import { ProjectModel } from '../../model/project/project-model';
import { CoreModuleModelImpl } from '../../model/module/core-module-model-impl';
import { AbstractHapModulePlugin } from '../common/abstract-hap-module-plugin';
import { AbstractHarModulePlugin } from '../common/abstract-har-module-plugin';
import { AbstractProjectPlugin } from '../common/abstract-project-plugin';
import { AbstractPluginBuilder } from '../builder/abstract-plugin-builder';
/**
 * 抽象类，用于定义初始化策略的接口
 */
export declare abstract class InitializationStrategy {
    /**
     * 创建App插件
     *
     * @param project - 工程对象
     * @returns 返回一个抽象的项目插件。
     */
    abstract createAppPlugin(project: Project): AbstractProjectPlugin;
    /**
     * 创建HAP模块插件
     *
     * @param module - 模块对象
     * @param pluginBuilder - 插件构建器对象，用于构建HAP模块插件。
     * @returns 返回一个抽象的HAP模块插件。
     */
    abstract createHapPlugin(module: Module, pluginBuilder: AbstractPluginBuilder<AbstractHapModulePlugin>): AbstractHapModulePlugin;
    /**
     * 创建HAR模块插件
     *
     * @param module - 模块对象
     * @param pluginBuilder - 插件构建器对象，用于构建HAR模块插件。
     * @returns 返回一个抽象的HAR模块插件。
     */
    abstract createHarPlugin(module: Module, pluginBuilder: AbstractPluginBuilder<AbstractHarModulePlugin>): AbstractHarModulePlugin;
    /**
     * 创建项目模型
     *
     * @param project - 项目对象
     * @returns 返回项目模型
     */
    abstract createProjectModel(project: Project): ProjectModel;
    /**
     * 创建模块模型
     *
     * @param projectModel - 项目模型对象
     * @param module - 模块对象
     * @param isHapModule - 布尔值，指示是否为HAP模块
     * @returns 返回核心模块模型实现
     */
    protected createModuleModel(projectModel: ProjectModel, module: Module, isHapModule: boolean): CoreModuleModelImpl;
}
