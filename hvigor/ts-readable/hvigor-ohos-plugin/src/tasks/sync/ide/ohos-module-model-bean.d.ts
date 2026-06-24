/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sync/ide/ohos-module-model-bean.d.ts
 */
import { Module, ToolingModelBean, TaskInfo } from '@ohos/hvigor';
/**
 * OhosProject中的Module模块的toolModel数据模型
 *
 * @since 2022/2/19
 */
export declare class OhosModuleModelBean implements ToolingModelBean {
    modelId: string;
    private readonly _isFaMode;
    private readonly _module;
    private readonly _projectModel;
    private readonly _moduleModel;
    private readonly _moduleTaskService;
    private readonly _targetDataSet;
    private readonly _isHarModule;
    private readonly _targetTaskServices;
    private readonly _moduleType;
    constructor(module: Module, isFaMode: boolean);
    private computeCommonInfo;
    private computeBuildOption;
    private computePathInfo;
    private getCachePath;
    private getFirstEntryName;
    private computeBuildDirInfo;
    buildModel(): object;
    taskInfo(): TaskInfo[];
}
