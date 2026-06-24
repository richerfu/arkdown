/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/task/default-task.d.ts
 */
import { TaskDetails } from '../../internal/task/interface/task-details-interface';
import { HvigorCoreNode } from '../core/hvigor-core-node';
import { Registry } from './core-task';
import { CoreTaskImpl } from './core-task-impl';
/**
 * Hvigor自定义任务的基础抽象类
 *
 * @since 2022/4/24
 */
export declare abstract class DefaultTask extends CoreTaskImpl implements Registry {
    /**
     * 任何自定义的任务类都需要实现此构造方法，分别为指定模块的Node,以及对应的taskInfo
     * taskInfo可以只指定一个taskName，其他为默认值,或者为一个TaskDetails的对象
     *
     * @param {HvigorCoreNode} node
     * @param {string | TaskDetails} taskInfo
     * @protected
     */
    protected constructor(node: HvigorCoreNode, taskInfo: string | TaskDetails);
    /**
     * 注册task function
     *
     * @return {Function} 任务需要执行的逻辑
     */
    registryAction(): Function;
    /**
     * 获取task的执行逻辑函数
     *
     * @return {Function} 任务需要执行的逻辑
     */
    getAction(): Function;
}
