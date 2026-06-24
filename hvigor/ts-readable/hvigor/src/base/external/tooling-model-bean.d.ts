/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/tooling-model-bean.d.ts
 */
import { TaskInfo } from './interface/bean-interface';
/**
 * Hvigor扩展数据模型的接口类型定义
 *
 * @since 2022/2/19
 */
export interface ToolingModelBean {
    modelId: string;
    /**
     * 调用相关的函数，来初始化和注册对应的ToolingModelBean
     *
     * @return {object}
     */
    buildModel(): object;
    /**
     * 输出任务的TaskInfo列表
     *
     * @return {object}
     */
    taskInfo(): TaskInfo[];
}
