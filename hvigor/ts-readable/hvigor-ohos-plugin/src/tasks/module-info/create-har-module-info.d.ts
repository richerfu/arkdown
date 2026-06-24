/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/module-info/create-har-module-info.d.ts
 */
import { TargetTaskService } from '../service/target-task-service';
import { OhosHapTask } from '../task/ohos-hap-task';
/**
 * 字节har要在generated目录生成一个ModuleInfo.ts文件，用于后续操作
 * 非字节码har要根据用户配置的开关决定是否生成ModuleInfo.ts文件
 * 该任务的主要逻辑就是生成ModuleInfo.ts文件，并写入MODULE_INFO_CODE内容
 */
export declare class CreateHarModuleInfo extends OhosHapTask {
    private readonly outputFile;
    constructor(taskService: TargetTaskService);
    private get isByteCodeHar();
    private get isOhosUiTransformOptimization();
    taskShouldDo(): boolean;
    protected beforeTask(): void;
    protected doTaskAction(): Promise<void>;
    initTaskDepends(): void;
}
