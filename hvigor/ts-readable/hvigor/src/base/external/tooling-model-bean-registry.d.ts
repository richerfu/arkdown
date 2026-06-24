/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/tooling-model-bean-registry.d.ts
 */
import { ToolingModelBean } from './tooling-model-bean';
/**
 * Hvigor的数据扩展模型的注册器接口定义
 *
 * @since 2022/2/19
 */
export interface ToolingModelBeanRegistry {
    /**
     * 注册对应的bean对象到当前Node的容器中进行保存
     *
     * @param {ToolingModelBean} modelBean
     */
    registry(modelBean: ToolingModelBean): void;
    /**
     * 提供查询容器中的所有的bean对象的map
     *
     * @return {Map<string, ToolingModelBean>}
     */
    getModelMap(): Map<string, ToolingModelBean>;
}
