/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/factory/core-plugin-builder.d.ts
 */
import { Module } from '@ohos/hvigor';
import { CoreModuleModelImpl } from '../../model/module/core-module-model-impl';
import { ProjectModel } from '../../model/project/project-model';
/**
 * PluginBuilder的公共基类
 *
 * @since 2023/02/01
 */
export declare class CorePluginBuilder {
    protected readonly module: Module;
    constructor(module: Module);
    createModuleModel(projectModel: ProjectModel, isHapModule: boolean): CoreModuleModelImpl;
}
