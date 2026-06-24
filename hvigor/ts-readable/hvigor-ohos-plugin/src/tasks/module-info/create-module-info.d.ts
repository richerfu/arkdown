/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/module-info/create-module-info.d.ts
 */
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
export declare const MODULE_INFO_CODE = "\n//@ts-noCheck\nexport const __MODULE_NAME__: string = __getCurrentModuleName__()\nexport const __BUNDLE_NAME__: string = __getCurrentBundleName__()\n";
/**
 * 要在generated目录生成一个ModuleInfo.ts文件，用于后续操作
 * 该任务的主要逻辑就是生成ModuleInfo.ts文件，并写入MODULE_INFO_CODE内容
 * 此任务主要用于hap/hsp模块，CreateHarModuleInfo任务主要用于处理har模块
 */
export declare class CreateModuleInfo extends OhosHapTask {
    private readonly outputFile;
    constructor(taskService: TargetTaskService);
    protected beforeTask(): void;
    private get ohosUiTransformOptimization();
    private get integratedHspUseGetCurrentBundleName();
    taskShouldDo(): boolean;
    protected doTaskAction(): Promise<void>;
    initTaskDepends(): void;
}
