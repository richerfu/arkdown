/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/builder/abstract-plugin-builder.d.ts
 */
import { HvigorCoreNode } from '@ohos/hvigor';
import { TaskCreatorManager } from '../../tasks/task-creator';
import { InitializationStrategy } from '../strategy/initialization-strategy';
import { OhosLogger } from '../../utils/log/ohos-logger';
/**
 * 插件构建器抽象基类，定义插件构建的标准流程
 *
 * @template T 插件类型
 */
export declare abstract class AbstractPluginBuilder<T> {
    protected readonly _log: OhosLogger;
    protected isFaMode: boolean;
    protected initializationStrategy: InitializationStrategy;
    constructor(isFaMode: boolean);
    /**
     * 构建插件的模板方法，定义了构建流程
     *
     * @returns {T} 返回构建好的插件实例
     */
    build(): T;
    /**
     * 创建插件前的准备逻辑
     *
     * @returns {void} 无返回值
     */
    protected prepare(): void;
    /**
     * 创建插件的抽象方法
     * 子类必须实现此方法以返回具体的插件实例
     *
     * @returns {T} 返回创建的插件实例
     */
    protected abstract createPlugin(): T;
    /**
     * 注册插件钩子的抽象方法
     * 子类必须实现此方法以定义如何注册插件的钩子
     *
     * @param {T} plugin - 需要注册钩子的插件实例
     * @returns {void} 无返回值
     */
    protected abstract registerHooks(plugin: T): void;
    /**
     * 初始化任务依赖信息
     *
     * @param module 模块
     * @param creationManager 依赖管理器
     * @param isFa 是否为fa
     * @returns {void} 无返回值
     */
    protected initializeTaskDepends(module: HvigorCoreNode, creationManager?: TaskCreatorManager, isFa?: boolean): void;
    /**
     * 通过公共参数判断是否要对未存在的buildModeInfo报错
     *
     * @returns {void} 无返回值
     */
    protected ifThrowBuildModeError(): void;
    /**
     * 根据模式创建初始化策略
     * 此方法根据当前模式判断并返回相应的初始化策略实例。
     *
     * @returns {InitializationStrategy} 返回初始化策略实例，若为FA模式则返回FaInitializationStrategy，否则返回DefaultInitializationStrategy。
     */
    private createInitializationStrategy;
}
