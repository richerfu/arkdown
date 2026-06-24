/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/builder/app-plugin-builder.d.ts
 */
import { Project } from '@ohos/hvigor';
import { AbstractProjectPlugin } from '../common/abstract-project-plugin';
import { AbstractPluginBuilder } from './abstract-plugin-builder';
export declare class AppPluginBuilder extends AbstractPluginBuilder<AbstractProjectPlugin> {
    private readonly project;
    constructor(project: Project, isFaMode: boolean);
    protected prepare(): void;
    protected createPlugin(): AbstractProjectPlugin;
    protected registerHooks(appPlugin: AbstractProjectPlugin): void;
    /**
     * 注册初始化全局数据、构建选项映射、构建上下文等逻辑到系统插件结束后的hook点
     *
     * @param appPlugin app插件实例
     */
    private registerAfterBindSystemPluginsHook;
    /**
     * 注册依赖解析等逻辑到节点评估完成后的钩子方法
     *
     * @param appPlugin 应用插件实例
     */
    private registerNodeEvaluatedInternalHook;
    /**
     * 注册ohpm加载安装钩子
     * 此方法用于在项目构建过程中，当oh-package.json文件发生修改时，
     * 将修改内容同步到build文件夹中。
     *
     * @param appPlugin 抽象项目插件实例
     */
    private registerOhpmLoadInstallHook;
    /**
     * 进行模块级增量跳过的预处理
     *
     * @param project 工程对象
     */
    private registerModuleSkipOptimizationHook;
    /**
     * 注册任务调度优化钩子
     * 此方法用于进行任务调度优化。
     *
     * @param project 工程对象
     */
    private registerTaskScheduleOptimizationHook;
}
