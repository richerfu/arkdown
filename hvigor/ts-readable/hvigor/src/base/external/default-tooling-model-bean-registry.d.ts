/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/default-tooling-model-bean-registry.d.ts
 */
import { ToolingModelBean } from './tooling-model-bean';
import { ToolingModelBeanRegistry } from './tooling-model-bean-registry';
/**
 * 默认的Hvigor的数据扩展模型的注册器
 *
 * @since 2022/2/19
 */
declare class DefaultToolingModelBeanRegistry implements ToolingModelBeanRegistry {
    private _log;
    private readonly _modelMap;
    constructor();
    registry(modelBean: ToolingModelBean): void;
    getModelMap(): Map<string, ToolingModelBean>;
    clear(): void;
}
export declare const defaultModelRegistry: DefaultToolingModelBeanRegistry;
export {};
